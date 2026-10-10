//! Строчный размер колонки для измерения строк.

use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::inline_content;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

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
        let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
        let b = e.style.borders();
        (!col_vert
            && matches!(e.style.width, None | Some(Len::Auto))
            && matches!(e.style.display, None | Some(Display::Block))
            && e.style.float.unwrap_or(0) == 0
            && !out_of_flow(&e.style)
            && [&e.style.margin.left, &e.style.margin.right, &e.style.padding.left, &e.style.padding.right, &b.left, &b.right]
                .into_iter()
                .all(zero)
            // Только простой поток: строки и листья с высотой в точках.
            // Блок с переполнением своей высоты (`css-break/block-max-
            // height-001-ref`: 160 с ребёнком 200) стопка рисует иначе,
            // чем прежний путь рисует тест с `max-height` (0.00 -> 11).
            && e.children.iter().all(|n| match n {
                Node::Text(_) => true,
                Node::Element(k) => {
                    k.inline
                        || inline_content(k)
                        || (k.children.iter().all(is_blank)
                            && matches!(k.style.height, Some(Len::Px(_))))
                }
            }))
        .then_some(inherited.width)
        .flatten()
        .filter(|w| matches!(w, Len::Px(_)))
    });

    match line_inline_size {
        Some(Len::Px(u)) if merged.border_box != Some(true) && cols > 0 => {
            Some(((u + used_gap) / cols as f32 - used_gap).max(0.0))
        }
        _ => None,
    }
}
