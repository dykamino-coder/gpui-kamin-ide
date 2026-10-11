//! Стопка сетки (рядами в один столбец) и рост дорожки при переносе.

use super::super::grid_px_row_bands;
use crate::dom::{Element, Node};
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Сетка, которую фрагментация вправе спускать СТОПКОЙ: одна колонка, ряды
/// по содержимому, дети без явного размещения. Тогда ряд — ровно один
/// ребёнок, высота ряда равна мере ребёнка (css-grid-1 §11.8: дорожка
/// `auto` — по max-content), а порядок рядов равен порядку детей (§8.5
/// auto-placement при `grid-auto-flow: row`). Поля рядов НЕ схлопываются
/// (§6.1: «margins of grid items do not collapse»), между рядами стоит
/// `row-gap`.
///
/// Отказ (прежний путь — `grid_rows_px`, чаще всего `None`): явные дорожки
/// рядов не все `auto` (px/`fr`/`minmax` — размер ряда не равен мере
/// ребёнка), колонок больше одной (дети параллельны, а не стопкой),
/// именованные области, поток по колонкам или `dense`, неявные ряды
/// заданного размера, распределяющий `align-content` (двигает ряды внутри
/// заданной высоты), зазор не в точках, явное размещение у любого ребёнка
/// (`grid-row`/`grid-column`/`grid-area`). `display: grid-lanes` не
/// проходит никогда — у полос своя укладка.
pub(crate) fn grid_stack(c: &Element) -> bool {
    use crate::style::computed::{AutoFlow, Track, TrackSize};
    let s = &c.style;
    if !matches!(s.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return false;
    }
    if s.grid_cols.unwrap_or(1) > 1
        || s.grid_tracks.as_ref().is_some_and(|t| t.len() > 1)
        || s.grid_areas.is_some()
    {
        return false;
    }
    if let Some(rows) = s.grid_rows.as_ref() {
        // Ряд, чей размер при `height: auto` равен вкладу ЕДИНСТВЕННОГО
        // элемента ряда: `auto`/`min-content`/`max-content` (css-grid-1
        // §12.4-12.6); `minmax(<0 | по содержимому>, <auto | max-content |
        // fr>)` — база не больше вклада, предел = вклад; `fr` при
        // неопределённом свободном месте — §12.7.1 «max-content contribution»
        // (ровно вклад, пока гибкая дорожка ОДНА); `minmax(<по содержимому>,
        // <px>)` — база = min-content = вклад (расходится лишь для элемента
        // ниже предела; копия кладётся `Definite(h)` и берёт ту же высоту).
        // css-grid-2 §12.1 шаг 3 растит именно такие ряды. С заданной
        // высотой `fr`/`minmax` делят ЕЁ, а не вклад: `grid-item-
        // fragmentation-014/016` (`height:200px`) держатся на прежнем пути.
        let content = |t: &Track| matches!(t, Track::Auto | Track::MinContent | Track::MaxContent);
        let lo_ok = |t: &Track| content(t) || matches!(t, Track::Px(v) if *v <= 0.0);
        let by_item = |t: &TrackSize| match t {
            TrackSize::Single(t) => content(t) || matches!(t, Track::Fr(_)),
            TrackSize::MinMax(lo, hi) => {
                lo_ok(lo)
                    && (content(hi)
                        || matches!(hi, Track::Fr(_))
                        || (content(lo) && matches!(hi, Track::Px(_))))
            }
            TrackSize::AutoRepeat { .. } => false,
        };
        let all_auto = rows
            .iter()
            .all(|t| matches!(t, TrackSize::Single(Track::Auto)));
        let flexible = rows
            .iter()
            .filter(|t| {
                matches!(
                    t,
                    TrackSize::Single(Track::Fr(_)) | TrackSize::MinMax(_, Track::Fr(_))
                )
            })
            .count();
        if !all_auto
            && (!matches!(s.height, None | Some(Len::Auto))
                || s.max_height.is_some()
                || flexible > 1
                || !rows.iter().all(by_item))
        {
            return false;
        }
    }
    if !matches!(
        s.grid_auto_rows,
        None | Some(TrackSize::Single(Track::Auto))
    ) || !s.grid_auto_rows_list.is_empty()
        || matches!(
            s.grid_auto_flow,
            Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
        )
        || s.align_content.is_some()
    {
        return false;
    }
    if !matches!(s.gap, None | Some((None, _)) | Some((Some(Len::Px(_)), _))) {
        return false;
    }
    !c.children.iter().any(|n| match n {
        Node::Element(k) => {
            k.style.grid_row.is_some()
                || k.style.grid_col.is_some()
                || k.style.grid_area_name.is_some()
        }
        _ => false,
    })
}

/// «Сдвиг ряда» сетки с рядами в точках (Blink `row_offset_adjustments`,
/// grid_layout_algorithm.cc:2304-2337: ряд, начатый в следующем
/// фрагментаинере, сдвигается на остаток предыдущего): разрез ровно на начале
/// ряда `i ≥ 1` (монолитный ряд или перенесённый `break-*`) растит дорожку
/// `i−1` на `grow`. Высота `auto` вырастает на то же (`grid_rows_px`),
/// заданная — нет, и ряд `i` всё равно встаёт на край колонки
/// (`grid-item-oof-004`: абсолют `align-self: end` в ряду 2 — в колонке 2).
/// Только без `row-gap`: при зазоре эталоны css-gaps держат ряд прежней
/// высоты (`grid-gap-decorations-fragmentation-011`). Прямой ребёнок стопки;
/// вложенная сетка — как прежде, без роста.
pub(crate) fn grow_grid_track(c: &mut Element, at: f32, grow: f32) -> bool {
    use crate::style::computed::{Track, TrackSize};
    if grid_stack(c)
        || !matches!(
            c.style.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
    {
        return false;
    }
    let gap0 = match c.style.gap {
        None | Some((None, _)) => true,
        Some((Some(Len::Px(v)), _)) => v.abs() < 0.01,
        _ => false,
    };
    if !gap0 {
        return false;
    }
    let Some(bands) = grid_px_row_bands(&c.style) else {
        return false;
    };
    let px_of = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let top = px_of(&c.style.padding.top) + px_of(&c.style.borders().top);
    let Some(i) = bands.iter().position(|b| (top + b.0 - at).abs() < 0.01) else {
        return false;
    };
    if i == 0 {
        return false;
    }
    let Some(TrackSize::Single(Track::Px(v))) =
        c.style.grid_rows.as_mut().and_then(|r| r.get_mut(i - 1))
    else {
        return false;
    };
    *v += grow;
    true
}
