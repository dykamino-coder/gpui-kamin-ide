//! Стопка колонок: строчный размер одной колонки для меры строк (`with_lines`).
//!
//! css-multicol-1 §3.4 (11): `W := max(0, (U + column-gap)/N − column-gap)` при
//! U в точках. Когда ширина многоколоночника `auto`, U берётся как ширина
//! содержимого родителя (CSS 2.1 §10.3.3) — только для простого потока без
//! боковых полей, рамок и отбивок; иначе строки не меряются (`None`). Раскладке копии
//! таблицы-ребёнка нужна та же ширина колонки без условия простого потока
//! (`column_layout_width`): её ряды кладёт сама раскладка, а не мера строк.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::flow::blocks::table_tree;
use crate::layout::fragment::line_shape::{inline_content, nested_rows_box};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Ширина колонки для раскладки копии ребёнка до укладки: css-multicol-1
/// §3.4 (11) при U в точках — заданной или `auto` блока потока без боковых
/// полей, рамок и отбивок (CSS 2.1 §10.3.3: ширина содержимого родителя).
pub(crate) fn column_layout_width(
    e: &Element,
    inherited: &Computed,
    merged: &Computed,
    col_inline_size: Option<Len>,
    col_vert: bool,
    cols: u16,
    used_gap: f32,
) -> Option<f32> {
    let u = col_inline_size.or_else(|| {
        (!col_vert && auto_fills_parent(e))
            .then_some(inherited.width)
            .flatten()
    });
    column_of(u, merged, cols, used_gap)
}

/// Ширина раскладки копии ребёнка до укладки: таблице из обычных блоков
/// (`table_tree`) — ширина колонки и без условия простого потока
/// (`column_layout_width`), её ряды кладёт раскладка; прочим — ширина меры
/// строк.
pub(crate) fn kid_layout_w(
    c: &Element,
    line_col_w: Option<f32>,
    layout_col_w: Option<f32>,
) -> Option<f32> {
    if table_tree(c, 4) {
        line_col_w.or(layout_col_w)
    } else {
        line_col_w
    }
}

/// Блок потока с `auto` шириной без боковых полей, рамок и отбивок: его
/// ширина — ширина содержимого родителя (CSS 2.1 §10.3.3).
fn auto_fills_parent(e: &Element) -> bool {
    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
    let b = e.style.borders();
    matches!(e.style.width, None | Some(Len::Auto))
        && matches!(e.style.display, None | Some(Display::Block))
        && e.style.float.unwrap_or(0) == 0
        && !out_of_flow(&e.style)
        && [
            &e.style.margin.left,
            &e.style.margin.right,
            &e.style.padding.left,
            &e.style.padding.right,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(zero)
}

/// Колонка из строчного размера коробки `u` (css-multicol-1 §3.4 (11)).
fn column_of(u: Option<Len>, merged: &Computed, cols: u16, used_gap: f32) -> Option<f32> {
    match u {
        Some(Len::Px(u)) if merged.border_box != Some(true) && cols > 0 => {
            Some(((u + used_gap) / cols as f32 - used_gap).max(0.0))
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn column_measure_width(
    e: &Element,
    inherited: &Computed,
    merged: &Computed,
    col_inline_size: Option<Len>,
    col_vert: bool,
    cols: u16,
    used_gap: f32,
) -> Option<f32> {
    // Строчный размер колонки для меры строк (`with_lines`):
    // css-multicol-1 §3.4 (11) «W := max(0, (U + column-gap)/N −
    // column-gap)» при U в точках; иначе строки не меряются.
    // Ширина `auto` блока в потоке — ширина содержимого родителя
    // (CSS 2.1 §10.3.3), когда та в точках и у коробки нет боковых
    // полей, рамок и отбивок (`text-box-trim-multicol-011-ref`:
    // многоколоночник без ширины в `.container` 640).
    let line_inline_size = col_inline_size.or_else(|| {
        (!col_vert
            && auto_fills_parent(e)
            // Только простой поток: строки и листья с высотой в точках.
            // Блок с переполнением своей высоты (`css-break/block-max-
            // height-001-ref`: 160 с ребёнком 200) стопка рисует иначе,
            // чем прежний путь рисует тест с `max-height`.
            && e.children.iter().all(|n| match n {
                Node::Text(_) => true,
                Node::Element(k) => {
                    k.inline
                        || inline_content(k)
                        || (k.children.iter().all(is_blank)
                            && matches!(k.style.height, Some(Len::Px(_))))
                        // Вложенный многоколоночник: его ширина — колонка
                        // внешнего, и мерить его раскладкой копии можно
                        // только при известной ширине (`measured_nested`).
                        || (nested_rows_box(k)
                            && matches!(k.style.height, None | Some(Len::Auto)))
                }
            }))
        .then_some(inherited.width)
        .flatten()
        .filter(|w| matches!(w, Len::Px(_)))
    });
    column_of(line_inline_size, merged, cols, used_gap)
}
