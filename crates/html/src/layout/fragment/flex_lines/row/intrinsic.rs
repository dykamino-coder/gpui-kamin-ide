//! Известные вклады вложенного flex-ряда в главный размер (css-flexbox-1 §9.9): max-content —
//! сумма внешних ширин элементов с зазорами, min-content — наибольшая из них; доли, `flex-basis`
//! и ограничения ширины требуют циклического разрешения и дают `None`.

use super::super::items::row_item_width;
use super::geometry::row_container;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::style::values::value::{Len, px_auto_zero};

pub(super) fn intrinsic_main(c: &Element, main: f32, depth: u8) -> Option<(f32, f32)> {
    if depth == 0
        || !row_container(c)
        || c.style.cascaded().min_width.is_some()
        || c.style.cascaded().max_width.is_some()
    {
        return None;
    }
    let (mut maximum, mut minimum) = (0.0f32, 0.0f32);
    let gap = px_auto_zero(c.style.cascaded().gap.and_then(|g| g.1))?.max(0.0);
    let mut first = true;
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        if out_of_flow(k.style.cascaded()) {
            continue;
        }
        // Percentages in an intrinsic contribution need cyclic sizing resolution.
        if k.style.cascaded().min_width.is_some()
            || k.style.cascaded().max_width.is_some()
            || !matches!(k.style.cascaded().flex_basis, None | Some(Len::Auto))
            || matches!(k.style.cascaded().width, Some(Len::Pct(_)))
            || matches!(k.style.cascaded().flex_basis, Some(Len::Pct(_)))
            || k.style.cascaded().flex_grow.is_some_and(|g| g > 0.0)
        {
            return None;
        }
        let (mx, mn) = match row_item_width(k.style.cascaded(), main) {
            Some(w) => (w, w),
            None => intrinsic_main(k, main, depth - 1)?,
        };
        let b = k.style.cascaded().borders();
        let decoration = px_auto_zero(k.style.cascaded().padding.left)?
            + px_auto_zero(k.style.cascaded().padding.right)?
            + px_auto_zero(b.left)?
            + px_auto_zero(b.right)?;
        let edges = if k.style.cascaded().border_box == Some(true) {
            0.0
        } else {
            decoration
        } + px_auto_zero(k.style.cascaded().margin.left)?
            + px_auto_zero(k.style.cascaded().margin.right)?;
        maximum += mx + edges;
        minimum = minimum.max(mn + edges);
        if !first {
            maximum += gap;
        }
        first = false;
    }
    let b = c.style.cascaded().borders();
    let edges = px_auto_zero(c.style.cascaded().padding.left)?
        + px_auto_zero(c.style.cascaded().padding.right)?
        + px_auto_zero(b.left)?
        + px_auto_zero(b.right)?;
    let add = if c.style.cascaded().border_box == Some(true) {
        edges
    } else {
        0.0
    };
    Some((maximum.max(0.0) + add, minimum.max(0.0) + add))
}
