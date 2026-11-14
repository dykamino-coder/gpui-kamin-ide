//! The paint-only path of a restyle: changed paint values written into the
//! document's scene layer (`gpui::LayerHandle`) instead of a new frame.
//!
//! A restyle whose every element changed only values the layer takes
//! (`PaintValue::layer_writable`: in R1 the colour of its own background)
//! leaves layout, the element tree and every other painted value as they
//! are: the recorded frame stays, and the layer takes the new colours through
//! the quads the boxes' background scopes bound (`Div::paint_owner`). Blink
//! reaches the same state through paint invalidation without layout
//! (`object_paint_invalidator.cc`), then repaints the chunk's display items;
//! WebRender rewrites a `PropertyBinding` colour without a new scene
//! (`picture.rs` color bindings). Here no item is painted again: the write
//! touches the bound quads only. Any box whose painted colour is not the one
//! its old style gives (a custom painter, a hover layer, an unbound fill)
//! refuses the write, and the document builds its frame as before.

use super::paint_damage::{PaintDamage, PaintValue, classify};
use crate::dom::{Element, Node};
use crate::style::computed::BoxStyle;
use gpui::{Bounds, Hsla, LayerHandle, ScaledPixels};

/// Write the paint values of the restyled elements `ids` (box numbers) from
/// the tree `old` to the tree `new` into `layer`. `None`, with nothing
/// written, when an element's damage is not a layer write or a painted fill
/// does not take it; otherwise the damaged rectangle (`None` inside when no
/// painted value changed).
pub(super) fn write_paint_values(
    old: &[Node],
    new: &[Node],
    ids: &[u64],
    layer: &LayerHandle,
) -> Option<Option<Bounds<ScaledPixels>>> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let (before, after) = (boxes(old, &ids)?, boxes(new, &ids)?);
    if before.len() != after.len() {
        return None;
    }
    let mut writes: Vec<Write> = Vec::with_capacity(ids.len());
    for (a, b) in before.iter().zip(&after) {
        let PaintDamage::Value(damage) = classify(a, b) else {
            return None;
        };
        if !damage.layer_writable() {
            return None;
        }
        if !damage.contains(PaintValue::BACKGROUND_COLOR) {
            continue;
        }
        let owner = u32::try_from(a.node_id).ok()?;
        let (from, to) = (painted_background(a)?, painted_background(b)?);
        // Fragments of one element share an owner: one write each.
        if writes.last().is_none_or(|w: &Write| w.0 != owner) {
            writes.push((owner, from, to));
        }
    }
    layer.set_backgrounds(&writes)
}

/// An owner's fill before and after (`None`: no fill).
type Write = (u32, Option<Hsla>, Option<Hsla>);

/// The boxes of the elements `ids` (sorted), in tree order; `None` unless
/// every element has one in `nodes`.
fn boxes<'a>(nodes: &'a [Node], ids: &[u64]) -> Option<Vec<&'a Element>> {
    let (low, high) = (*ids.first()?, *ids.last()?);
    let mut found = Vec::new();
    // Pre-order without recursion: the tree's depth is bounded only by the
    // representation's limit.
    let mut stack: Vec<std::slice::Iter<'a, Node>> = vec![nodes.iter()];
    while let Some(list) = stack.last_mut() {
        let Some(node) = list.next() else {
            stack.pop();
            continue;
        };
        let Node::Element(e) = node else { continue };
        if (low..=high).contains(&e.node_id) && ids.binary_search(&e.node_id).is_ok() {
            found.push(e);
        }
        stack.push(e.children.iter());
    }
    let every = ids.iter().all(|id| found.iter().any(|e| e.node_id == *id));
    every.then_some(found)
}

/// The plain colour the box's background paints (`Some(None)`: no fill);
/// `None` for a gradient, an image stack, a flattened stack or a colour
/// clipped apart from the box (`background-clip`), which no single fill of
/// the box's own painter (`Style::paint_box`) carries.
fn painted_background(e: &Element) -> Option<Option<Hsla>> {
    let style = BoxStyle::shared(e.style.used());
    let values = style.paint_background();
    let plain = values.gradient.is_none()
        && values.gradient_image.is_none()
        && values.layers.is_none()
        && values.flat.is_none()
        && values.color_clip.is_none();
    plain.then(|| values.color.map(|c| c.to_hsla()))
}
