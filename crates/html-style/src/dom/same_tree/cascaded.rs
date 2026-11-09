//! Value equality of two cascades of one source DOM by their cascaded values.
//!
//! A computed style is a function of the element's cascaded style, its
//! parent's computed style and the document inputs (viewport, fonts). Two
//! cascades of the same source DOM whose subtrees carry equal cascaded values
//! under equal parents therefore compute equal values throughout, so a
//! rebuild can compare them before its styles are computed (Blink
//! `StyleDifference` compares the old and new style of each object at
//! recalc, before layout). Paint-only values of an ancestor reach the
//! subtree only as paint-only values (`Computed::same_layout`).

use super::{Element, SharedStyle, layers, same_shape, walk};

/// The element styles of `a` and `b` cascade equal values (`layout`: equal
/// but for paint-only values), in every style stage.
pub fn same_cascaded_box(a: &Element, b: &Element, layout: bool) -> bool {
    let style = |x: &SharedStyle, y: &SharedStyle| {
        let (x, y) = (x.cascaded(), y.cascaded());
        std::ptr::eq(x, y) || if layout { x.same_layout(y) } else { x == y }
    };
    let layer = |x: Option<&SharedStyle>, y: Option<&SharedStyle>| match (x, y) {
        (None, None) => true,
        (Some(x), Some(y)) => style(x, y),
        _ => false,
    };
    a.marks == b.marks
        && style(&a.style, &b.style)
        && layers(a)
            .into_iter()
            .zip(layers(b))
            .all(|(x, y)| layer(x, y))
}

/// The subtrees of `a` and `b` have the same node ids, structure, texts and
/// attributes, and cascade equal values throughout (`layout`: equal but for
/// paint-only values; see the module).
pub fn same_cascaded_tree(a: &Element, b: &Element, layout: bool) -> bool {
    fn exact(a: &Element, b: &Element) -> bool {
        same_cascaded_box(a, b, false)
    }
    fn layout_only(a: &Element, b: &Element) -> bool {
        same_cascaded_box(a, b, true)
    }
    let boxes: fn(&Element, &Element) -> bool = if layout { layout_only } else { exact };
    a.node_id == b.node_id
        && walk(same_shape(a, b), &a.children, b, same_shape)
        && walk(boxes(a, b), &a.children, b, boxes)
}
