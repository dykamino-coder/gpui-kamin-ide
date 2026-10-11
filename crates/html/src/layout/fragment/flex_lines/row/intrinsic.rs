//! Known intrinsic contributions of nested flex rows, CSS Flexbox section 9.9.

use super::super::items::row_item_width;
use super::geometry::{px, row_container};
use crate::dom::{Element, Node};
use crate::render::{is_blank, out_of_flow};
use crate::style::values::value::Len;

pub(super) fn intrinsic_main(c: &Element, main: f32, depth: u8) -> Option<(f32, f32)> {
    if depth == 0 || !row_container(c) || c.style.min_width.is_some() || c.style.max_width.is_some()
    {
        return None;
    }
    let (mut maximum, mut minimum) = (0.0f32, 0.0f32);
    let gap = px(c.style.gap.and_then(|g| g.1))?.max(0.0);
    let mut first = true;
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        if out_of_flow(&k.style) {
            continue;
        }
        // Percentages in an intrinsic contribution need cyclic sizing resolution.
        if k.style.min_width.is_some()
            || k.style.max_width.is_some()
            || !matches!(k.style.flex_basis, None | Some(Len::Auto))
            || matches!(k.style.width, Some(Len::Pct(_)))
            || matches!(k.style.flex_basis, Some(Len::Pct(_)))
            || k.style.flex_grow.is_some_and(|g| g > 0.0)
        {
            return None;
        }
        let (mx, mn) = match row_item_width(&k.style, main) {
            Some(w) => (w, w),
            None => intrinsic_main(k, main, depth - 1)?,
        };
        let b = k.style.borders();
        let decoration =
            px(k.style.padding.left)? + px(k.style.padding.right)? + px(b.left)? + px(b.right)?;
        let edges = if k.style.border_box == Some(true) {
            0.0
        } else {
            decoration
        } + px(k.style.margin.left)?
            + px(k.style.margin.right)?;
        maximum += mx + edges;
        minimum = minimum.max(mn + edges);
        if !first {
            maximum += gap;
        }
        first = false;
    }
    let b = c.style.borders();
    let edges = px(c.style.padding.left)? + px(c.style.padding.right)? + px(b.left)? + px(b.right)?;
    let add = if c.style.border_box == Some(true) {
        edges
    } else {
        0.0
    };
    Some((maximum.max(0.0) + add, minimum.max(0.0) + add))
}
