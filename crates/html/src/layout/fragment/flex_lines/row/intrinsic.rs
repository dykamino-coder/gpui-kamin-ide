//! Известные вклады вложенного flex-ряда в главный размер (css-flexbox-1 §9.9): max-content —
//! сумма внешних ширин элементов с зазорами, min-content — наибольшая из них; доли, `flex-basis`
//! и ограничения ширины требуют циклического разрешения и дают `None`.

use super::super::items::row_item_width;
use super::container::row_container;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::box_kind::out_of_flow;
use crate::style::values::value::{Len, px_auto_zero};

pub(super) fn intrinsic_main(c: &Element, main: f32, depth: u8) -> Option<(f32, f32)> {
    if depth == 0
        || !row_container(c)
        || c.style.used().min_width.is_some()
        || c.style.used().max_width.is_some()
    {
        return None;
    }
    let (mut maximum, mut minimum) = (0.0f32, 0.0f32);
    let gap = px_auto_zero(c.style.used().gap.and_then(|g| g.1))?.max(0.0);
    let mut first = true;
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        if out_of_flow(&k.style.used()) {
            continue;
        }
        // Percentages in an intrinsic contribution need cyclic sizing resolution.
        if k.style.used().min_width.is_some()
            || k.style.used().max_width.is_some()
            || !matches!(k.style.used().flex_basis, None | Some(Len::Auto))
            || matches!(k.style.used().width, Some(Len::Pct(_)))
            || matches!(k.style.used().flex_basis, Some(Len::Pct(_)))
            || k.style.used().flex_grow.is_some_and(|g| g > 0.0)
        {
            return None;
        }
        let (mx, mn) = match row_item_width(&k.style.used(), main) {
            Some(w) => (w, w),
            None => intrinsic_main(k, main, depth - 1)?,
        };
        let b = k.style.used().borders();
        let decoration = px_auto_zero(k.style.used().padding.left)?
            + px_auto_zero(k.style.used().padding.right)?
            + px_auto_zero(b.left)?
            + px_auto_zero(b.right)?;
        let edges = if BoxDimensions::for_element(k, &k.style.used()).border_box {
            0.0
        } else {
            decoration
        } + px_auto_zero(k.style.used().margin.left)?
            + px_auto_zero(k.style.used().margin.right)?;
        maximum += mx + edges;
        minimum = minimum.max(mn + edges);
        if !first {
            maximum += gap;
        }
        first = false;
    }
    let b = c.style.used().borders();
    let edges = px_auto_zero(c.style.used().padding.left)?
        + px_auto_zero(c.style.used().padding.right)?
        + px_auto_zero(b.left)?
        + px_auto_zero(b.right)?;
    let add = if BoxDimensions::for_element(c, &c.style.used()).border_box {
        edges
    } else {
        0.0
    };
    Some((maximum.max(0.0) + add, minimum.max(0.0) + add))
}
