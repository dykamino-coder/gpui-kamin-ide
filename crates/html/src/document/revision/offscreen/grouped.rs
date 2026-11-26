//! The area of a box painted inside a composition group the layer kept: a
//! paragraph painted there measures no ink in window space
//! (`layout::retained::measured`), so neither its fragments nor its ink
//! place the box.
//!
//! A group's image holds everything painted in it, and the layer knows
//! where the image lands: the area of the group's root box
//! (`LayerHandle::holds_group_root`, the label of its outermost group, or
//! the group's bounds when the label was culled and the group showed
//! nothing). Any paint of a box the group holds lands there, its new band,
//! shadows and outline included, as Blink maps a box's visual rect through
//! the effect and clip nodes above it (`MapToVisualRectInAncestorSpace`)
//! and Gecko clips a frame's ink overflow by its ancestors' clips. A box
//! moved away from where its ancestor laid it out between the root and
//! itself is not bounded here (`below::moves`).

use super::below::moves;
use crate::dom::Element;
use gpui::{Bounds, LayerHandle, ScaledPixels};

/// The area of the innermost group root in `up` (outermost first) holding
/// box `b`, device pixels in the layer; `None` when no group root holds it
/// or a box between them moves.
pub(super) fn area(layer: &LayerHandle, up: &[&Element], b: &Element) -> Option<Bounds<ScaledPixels>> {
    let root = up.iter().rposition(|e| {
        u32::try_from(e.node_id).is_ok_and(|owner| layer.holds_group_root(owner))
    })?;
    if moves(b) || up[root + 1..].iter().any(|e| moves(e)) {
        return None;
    }
    layer.box_area(u32::try_from(up[root].node_id).ok()?)
}
