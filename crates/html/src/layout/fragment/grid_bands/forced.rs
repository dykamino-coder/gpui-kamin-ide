//! Принудительные разрывы рядов сетки, стопка сетки и рост дорожки.

use super::grid_px_row_bands;
use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::probe::size_monolith;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Принудительные разрывы, перенесённые с ЭЛЕМЕНТОВ сетки на границы РЯДОВ
/// (css-grid-2 §Fragmenting Grid Layout: «The 'break-before' and
/// 'break-after' properties on grid items are propagated to their grid
/// row»). Blink `grid_layout_algorithm.cc:1846-1857`:
/// `row_break_between[set_indices.begin] |= item_break_before`,
/// `[set_indices.end] |= item_break_after`, причём оба значения берутся
/// через `InitialBreakBefore`/`FinalBreakAfter` — то есть С ПОТОМКОВ, что у
/// нас делает `edge_break`. Смещения — от верха СОДЕРЖИМОГО коробки и всегда
/// на НАЧАЛЕ ряда, с которого продолжится следующий фрагмент: зазор перед
/// ним съедается разрывом (css-gaps-1 §fragmentation).
///
/// Разрыв перед ПЕРВЫМ рядом и после ПОСЛЕДНЕГО сюда не попадает: спека
/// отдаёт его контейнеру, и его переносит `edge_break`.
///
/// Размещение элементов по рядам считается только там, где оно однозначно
/// (css-grid-1 §8.5, поток `row` без `dense`): именованных областей нет,
/// `grid-row`/`grid-area` у детей нет, число колонок известно. Курсор идёт
/// по колонкам, `grid-column: N / span M` занимает M колонок и при
/// необходимости пинает курсор вперёд; не влезающий в остаток ряда элемент
/// начинает новый ряд. Любая непонятная форма — пустой список, а не догадка.
pub(crate) fn grid_row_forced(c: &Element) -> (Vec<f32>, Vec<(f32, f32)>) {
    use crate::style::computed::{AutoFlow, Placement};
    let s = &c.style;
    if matches!(
        s.grid_auto_flow,
        Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
    ) {
        return (Vec::new(), Vec::new());
    }
    let Some(bands) = grid_px_row_bands(s) else {
        return (Vec::new(), Vec::new());
    };
    let cols = s
        .grid_cols
        .map(|n| n.max(1) as usize)
        .or_else(|| s.grid_tracks.as_ref().map(|t| t.len().max(1)))
        .unwrap_or(1);
    // Явный ОДИН ряд элемента: `grid-row: N`, `N / N+1` или именованная
    // область в один ряд. `Some(None)` — ряд по курсору, `None` — форма,
    // которой мы не знаем (отказ целиком, как прежде).
    let own_row = |k: &Element| -> Option<Option<usize>> {
        if let Some(name) = &k.style.grid_area_name {
            let areas = s.grid_areas.as_ref()?;
            let hit: Vec<usize> = areas
                .iter()
                .enumerate()
                .filter(|(_, r)| r.iter().any(|x| x == name))
                .map(|(i, _)| i)
                .collect();
            return match hit.as_slice() {
                [r] => Some(Some(*r)),
                _ => None,
            };
        }
        match k.style.grid_row {
            None | Some((Placement::Auto, Placement::Auto)) => Some(None),
            Some((Placement::Line(a), Placement::Auto)) if a >= 1 => Some(Some((a - 1) as usize)),
            Some((Placement::Line(a), Placement::Line(b))) if a >= 1 && b == a + 1 => {
                Some(Some((a - 1) as usize))
            }
            _ => None,
        }
    };
    let mut items: Vec<(&Element, Option<usize>)> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else {
            return (Vec::new(), Vec::new());
        };
        if matches!(k.style.display, Some(Display::None)) || out_of_flow(&k.style) {
            continue;
        }
        let Some(r) = own_row(k) else {
            return (Vec::new(), Vec::new());
        };
        items.push((k, r));
    }
    // Курсор ниже занятых ячеек не знает: смесь явных рядов с курсорными
    // (css-grid-1 §8.5 шаги 2 и 4 зависят друг от друга) и курсорный элемент
    // при областях — отказ.
    let explicit = items.iter().filter(|(_, r)| r.is_some()).count();
    if (explicit > 0 && explicit < items.len()) || (explicit == 0 && s.grid_areas.is_some()) {
        return (Vec::new(), Vec::new());
    }
    let mut out: Vec<f32> = Vec::new();
    // Ряды с монолитным элементом (css-break-4 §4.1; Blink: элемент, не
    // влезший в остаток, — разрыв ПЕРЕД рядом, `MovePastBreakpoint`,
    // grid_layout_algorithm.cc:2161-2178). Первый ряд — как прежде: разрыв
    // перед ним принадлежит контейнеру.
    let mut mono: Vec<(f32, f32)> = Vec::new();
    let mut row = 0usize;
    let mut col = 0usize;
    for (k, fixed) in items {
        let r = match fixed {
            Some(r) => r,
            None => {
                let (line, span) = match k.style.grid_col {
                    None | Some((Placement::Auto, Placement::Auto)) => (None, 1usize),
                    Some((Placement::Span(m), Placement::Auto)) => (None, m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Span(m))) => (Some(a), m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Auto)) => (Some(a), 1usize),
                    Some((Placement::Line(a), Placement::Line(b))) => {
                        (Some(a.min(b)), (b - a).unsigned_abs().max(1) as usize)
                    }
                    _ => return (Vec::new(), Vec::new()),
                };
                if span > cols {
                    return (Vec::new(), Vec::new());
                }
                if let Some(a) = line {
                    if a < 1 {
                        return (Vec::new(), Vec::new());
                    }
                    let want = (a - 1) as usize;
                    if want + span > cols {
                        return (Vec::new(), Vec::new());
                    }
                    if want < col {
                        row += 1;
                    }
                    col = want;
                } else if col + span > cols {
                    row += 1;
                    col = 0;
                }
                let here = row;
                col += span;
                if col >= cols {
                    row += 1;
                    col = 0;
                }
                here
            }
        };
        if r >= bands.len() {
            if fixed.is_some() {
                continue;
            }
            break;
        }
        if r > 0 && edge_break(k, false) {
            out.push(bands[r].0);
        }
        if r + 1 < bands.len() && edge_break(k, true) {
            out.push(bands[r + 1].0);
        }
        if r > 0 && (k.style.break_inside_avoid || size_monolith(k)) {
            mono.push(bands[r]);
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    out.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    (out, mono)
}

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
