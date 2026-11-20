//! Paint-only edits wholly off the screen of boxes the scene layer cannot
//! record again: an inline box whose band the layer does not take, a box
//! whose decorations paint as elements of their own.
//!
//! Such an edit changes no pixel of the frame when every pixel the box can
//! paint, before or after, lies outside the area the frame showed. Gecko
//! unions a frame's ink overflow into the dirty region
//! (`RetainedDisplayListBuilder::ProcessFrame`), and Blink invalidates per
//! fragment under the cull rect (`paint_invalidator.cc`): off the screen
//! the region is empty and nothing is built. Here the frame is kept and the
//! edit waits (`layout::reuse::Wait`); a replay that shows its area builds
//! the frame, and any other frame paints the boxes anew.
//!
//! The area is conservative, and unknown means "on the screen":
//! - an inline box: its line fragments as the layer's frame painted them
//!   (`layout::retained::fragments`), in the slot of its container box and
//!   only while that slot's frame is the layer's; a box nested in a band
//!   owner takes its owner's fragments;
//! - a box with decorations: its run in the layer, the box with its ink;
//! - both grown by the reach of the shadows and outline of either style; a
//!   filter, a border image or a border shape reaches farther than this
//!   knows, and is never deferred.

use super::patch::{Path, element_at};
use crate::dom::{Element, Node};
use crate::layout::reuse::{Store, Wait};
use gpui::{Bounds, LayerHandle, ScaledPixels};
use reach::{grow, reach};

mod reach;

/// The area an edit off the screen can paint and why it waits for a frame.
pub(super) type FrameWait = (Bounds<ScaledPixels>, Wait);

/// Count the edits waiting for a frame by why they wait.
pub(super) fn count(frames: &[FrameWait]) {
    for (_, wait) in frames {
        let counter = match wait {
            Wait::Inline => &crate::paint_path::DEFERRED_INLINE,
            Wait::Decoration => &crate::paint_path::DEFERRED_DECORATION,
        };
        crate::paint_path::add(counter, 1);
    }
}

/// The area box `a` (at `path` in `old`) restyled to `b` can paint and why it
/// waits, when that area lies wholly off the area the layer's frame showed.
pub(super) fn waits(
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    layer: &LayerHandle,
    (old, path): (&[Node], &Path),
    a: &Element,
    b: &Element,
) -> Option<(Bounds<ScaledPixels>, Wait)> {
    let outset = reach(a)?.max(reach(b)?);
    let (area, wait) = if super::layer_band::records_band(a) {
        (
            inline_area(store, boxes, old, path, a, outset)?,
            Wait::Inline,
        )
    } else {
        let scale = store.shown_scale();
        let run = layer.box_area(u32::try_from(a.node_id).ok()?)?;
        (grow(run, outset * scale), Wait::Decoration)
    };
    store.misses_shown(area).then_some((area, wait))
}

/// The fragments of inline box `a` grown by `outset`, device pixels.
fn inline_area(
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    old: &[Node],
    path: &Path,
    a: &Element,
    outset: f32,
) -> Option<Bounds<ScaledPixels>> {
    fragments_of(store, boxes, old, path, a, |f, owner| f.ink(owner, outset))?
}

/// `f` of the fragments of inline box `a` (or of the nearest band owner
/// holding it) in the slot of its container and their owner, while they
/// describe the layer.
fn fragments_of<R>(
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    old: &[Node],
    path: &[usize],
    a: &Element,
    f: impl FnOnce(&crate::layout::retained::InlineFragments, u32) -> R,
) -> Option<R> {
    let mut owners = vec![a.node_id];
    // The first ancestor that is not an inline box contains the paragraph.
    let container = (1..path.len()).rev().find_map(|k| {
        let e = element_at(old, &path[..k])?;
        if super::layer_band::records_band(e) {
            owners.push(e.node_id);
            None
        } else {
            Some(e)
        }
    })?;
    let slot = boxes.slot(boxes.box_of(container.node_id)?)?;
    let fragments = slot.inline.as_deref()?;
    if !store.fragments_hold(fragments.stamp()) {
        return None;
    }
    let owner = owners
        .into_iter()
        .filter_map(|o| u32::try_from(o).ok())
        .find(|o| fragments.has(*o))?;
    Some(f(fragments, owner))
}

impl crate::document::Document {
    /// The line boxes of the fragments of the inline box at `path`, window
    /// pixels, as the last recorded frame placed them, read without a build
    /// (the geometry of `getClientRects()`). `None` when no frame describes
    /// them now (a frame laid out since, the box in a block it skipped, a
    /// paragraph painted under effects): a synchronous reader then lays out
    /// the document first, as a script's layout flush does.
    #[cfg_attr(not(test), allow(dead_code, reason = "read by the script binding"))]
    pub(crate) fn inline_rects(&self, path: &[usize]) -> Option<Vec<gpui::Bounds<gpui::Pixels>>> {
        let e = element_at(&self.nodes, path)?;
        if !super::layer_band::records_band(e) {
            return None;
        }
        let boxes = self.state.boxes.borrow();
        fragments_of(&self.reuse, &boxes, &self.nodes, path, e, |f, owner| {
            f.rects(owner).collect()
        })
    }
}
