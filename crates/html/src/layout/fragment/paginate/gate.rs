//! Gate of layout-driven pagination: which multicol children take it.

use super::candidates;
use crate::dom::{Element, Node};
use crate::layout::fragment::clone::monolith_box;
use crate::render::out_of_flow;
use crate::style::computed::Display;

/// Gate of the path: a block-level, non-monolithic child of a horizontal
/// multicol that is (or contains) a flex container and has at least one
/// candidate. Everything else keeps the shape-estimate path.
pub(crate) fn gate(c: &Element, col_w: Option<f32>, auto_fill: bool) -> bool {
    refusal(c, col_w, auto_fill).is_none()
}

/// Why `gate` refuses `c` (`None` — accepted).
pub(crate) fn refusal(c: &Element, col_w: Option<f32>, auto_fill: bool) -> Option<&'static str> {
    // Flex gaps adjoining a break are truncated (Blink
    // `UpdateOffsetAdjustmentForSuppressedRowGap`); struts do not model that.
    fn gapped(e: &Element, depth: u8) -> bool {
        (matches!(e.style.display, Some(Display::Flex) | Some(Display::Grid))
            && e.style.gap.is_some())
            || (depth > 0
                && e.children.iter().any(
                    |n| matches!(n, Node::Element(k) if !monolith_box(k) && gapped(k, depth - 1)),
                ))
    }
    fn has_flex(e: &Element, depth: u8) -> bool {
        e.style.display == Some(Display::Flex)
            || (depth > 0
                && e.children.iter().any(
                    |n| matches!(n, Node::Element(k) if !monolith_box(k) && has_flex(k, depth - 1)),
                ))
    }
    if c.inline
        || out_of_flow(&c.style)
        || !matches!(
            c.style.display,
            Some(Display::Block) | Some(Display::Flex) | None
        )
    {
        return Some("display");
    }
    if monolith_box(c)
        || crate::layout::multicol::spanner::multicol_container(&c.style)
        || !crate::layout::page::paged::visible_overflow(&c.style)
        || c.style.bdb_clone
        || c.style.column_span == Some(true)
    {
        return Some("box");
    }
    if !has_flex(c, 4) {
        return Some("no-flex");
    }
    // Lines opened as parallel flows keep that model: it fixes the line
    // breaking before fragmentation (css-flexbox-1 §12), a strut would rewrap.
    if auto_fill && crate::layout::fragment::flex_lines::explodes(c, col_w) {
        return Some("exploded");
    }
    if gapped(c, 4) {
        return Some("gap");
    }
    match candidates(c) {
        None => Some("subtree"),
        Some(v) if v.is_empty() => Some("no-cand"),
        Some(_) => None,
    }
}
