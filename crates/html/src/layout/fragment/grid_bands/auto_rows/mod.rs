//! Полосы авто-рядов сетки: проходы размещения элементов и высоты рядов.

use super::{grid_rows_px, grid_stack};
use crate::dom::Element;
use crate::layout::fragment::{GridSpot, ShapeCx};
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod pass;
pub(super) use pass::grid_pass;

/// Полосы рядов сетки, когда `grid_rows_px` бессилен: колонок больше одной,
/// ряд `auto` или ряд вовсе неявный. `(начало, конец)` каждого ряда от верха
/// содержимого; последний конец — высота сетки.
///
/// css-grid-2 §Fragmenting Grid Layout, «Sample Fragmentation Algorithm»
/// шаг 4: «If the grid height is ''auto'', the height of the grid should be
/// the sum of the final row sizes». Blink считает ровно это —
/// `grid_layout_algorithm.cc:370` `CalculateIntrinsicBlockSize`:
/// `layout_data.Rows().CalculateSetSpanSize() + border_scrollbar_padding
/// .BlockSum()`, без всякого условия «дорожки в точках».
///
/// Размер ряда: явная дорожка `Px` — как есть; `auto` и неявный ряд — по
/// НАИБОЛЬШЕМУ элементу ряда (css-grid-1 §12.5: `auto` как максимум —
/// max-content вклада), с полями: поля элементов сетки не схлопываются
/// (§6.1). Размещение — css-grid-1 §8.5: сперва элементы с ЯВНОЙ линией
/// ряда (шаг 2), затем курсор по рядам (шаг 4).
///
/// Отказ (`None`) — на всём, где догадка была бы неверной: `fr`, проценты,
/// `minmax`, `min-content`, `subgrid`, `repeat(auto-fill …)` в дорожках;
/// `grid-template-areas`; `grid-auto-flow` по колонкам или `dense`;
/// `grid-auto-rows` заданного размера; распределяющий `align-content`;
/// зазор не в точках; явная КОЛОНКА или охват рядов у ребёнка; ребёнок,
/// который сам себя измерить не даёт. Отказ = прежнее поведение, поэтому
/// ни одна пара, что мерится сегодня, этой функции не видит: она стоит
/// ПОСЛЕ `grid_rows_px` в той же ветке.
///
/// Точек разреза функция НЕ даёт нарочно. Класс A между рядами
/// (css-grid-2 §Fragmenting Grid Layout) — возможность, а не предпочтение:
/// Blink переносит ряд в следующий фрагментаинер только при принудительном
/// разрыве (`grid_layout_algorithm.cc:2161-2167`) или при отказе
/// `MovePastBreakpoint` (:2178), а обычный ряд режет по краю. Точка класса A
/// на каждой границе ряда увела бы разрез у зелёных
/// `grid-item-oof-002/003` (ряды `50px 150px`, край колонки на 100 внутри
/// второго ряда) с края на 50 и потеряла бы половину колонки.
pub(crate) fn grid_auto_row_bands(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
) -> Option<(Vec<(f32, f32)>, Vec<GridSpot>)> {
    use crate::style::computed::{AutoFlow, Track, TrackSize};
    let s = &c.style;
    if depth == 0 || !matches!(s.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return None;
    }
    // Именованные области размещаются ниже (область в один ряд); прочее —
    // отказ, как прежде.
    if s.align_content.is_some()
        || matches!(
            s.grid_auto_flow,
            Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
        )
        || !matches!(
            s.grid_auto_rows,
            None | Some(TrackSize::Single(Track::Auto))
        )
        || !s.grid_auto_rows_list.is_empty()
    {
        return None;
    }
    let gap = match s.gap {
        None | Some((None, _)) => 0.0,
        Some((Some(Len::Px(v)), _)) => v,
        _ => return None,
    };
    // Явные дорожки рядов: `Some(px)` — размер известен, `None` — ряд `auto`
    // и меряется содержимым. Всё прочее — отказ.
    let mut track: Vec<Option<f32>> = Vec::new();
    if let Some(rows) = s.grid_rows.as_ref() {
        for t in rows {
            match t {
                TrackSize::Single(Track::Px(v)) => track.push(Some(*v)),
                TrackSize::Single(Track::Auto) => track.push(None),
                _ => return None,
            }
        }
    }
    // Колонки: `grid-template-columns` перечислимым списком либо его нет —
    // тогда неявная колонка ровно одна.
    let cols = match (s.grid_cols, s.grid_tracks.as_ref()) {
        (Some(n), _) => n.max(1) as usize,
        (None, Some(t)) => {
            if t.iter().any(|x| matches!(x, TrackSize::AutoRepeat { .. })) {
                return None;
            }
            t.len().max(1)
        }
        (None, None) => 1,
    };
    // Области задают и неявные колонки (css-grid-1 §7.3): `'a b' 'c c'` без
    // `grid-template-columns` — две колонки.
    let cols = cols.max(
        s.grid_areas
            .as_ref()
            .map_or(0, |a| a.iter().map(|r| r.len()).max().unwrap_or(0)),
    );
    // `used[ряд][колонка]` — занятость, `fill[ряд]` — содержимое ряда.
    let mut used: Vec<Vec<bool>> = Vec::new();
    // Элементы с их рядом и мерой — для внутренних точек (`shape_full`) и
    // спуска распорки роста (`pushed_box_at`).
    let mut spots: Vec<GridSpot> = Vec::new();
    let mut fill: Vec<f32> = Vec::new();
    for pass in 0..2u8 {
        grid_pass(
            c, depth, cx, s, cols, &mut used, &mut spots, &mut fill, pass,
        )?;
    }
    let rows_n = track.len().max(used.len());
    if rows_n == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(rows_n);
    let mut y = 0.0f32;
    for i in 0..rows_n {
        let h = match track.get(i) {
            Some(Some(v)) => *v,
            _ => fill.get(i).copied().unwrap_or(0.0),
        };
        out.push((y, y + h));
        y += h + gap;
    }
    Some((out, spots))
}

/// Сетка, которую мерит `grid_auto_row_bands`, — та же цепочка, что в
/// `shape_full`: не стопка, высота `auto`, ряды не все в точках.
pub(crate) fn grid_items_spotted(c: &Element) -> bool {
    matches!(
        c.style.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) && !grid_stack(c)
        && c.style.height.is_none()
        && grid_rows_px(&c.style).is_none()
}
