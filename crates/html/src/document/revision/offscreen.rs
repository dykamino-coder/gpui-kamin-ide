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
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::{Bounds, LayerHandle, ScaledPixels};

/// The area an edit off the screen can paint and why it waits for a frame.
pub(super) type FrameWait = (Bounds<ScaledPixels>, Wait);

/// The refusals an edit wholly off the screen waits before: a box with
/// decorations, a band change that moves no layout. Another refusal, or a
/// box whose area is not known to be off the screen (`wait`), refuses
/// (`None`); otherwise the edit waits in `frames`.
pub(super) fn wait_or_refuse(
    reason: &'static str,
    wait: impl FnOnce() -> Option<FrameWait>,
    frames: &mut Vec<FrameWait>,
) -> Option<()> {
    let waits = matches!(
        reason,
        "decoration" | "band_paint" | "band_nested" | "band_geometry"
    );
    match waits.then(wait).flatten() {
        Some(w) => {
            frames.push(w);
            Some(())
        }
        None => {
            super::layer_write::refuse_not_own(reason);
            None
        }
    }
}

/// The `records` of boxes the layer holds (`held`); an inline box it does not
/// hold waits off the screen (`waits`) or refuses the path (`None`).
pub(super) fn unheld(
    (old, new): (&[Node], &[Node]),
    records: Vec<(Path, u64)>,
    held: impl Fn(u64) -> bool,
    waits: &impl Fn(&Path, &Element, &Element) -> Option<FrameWait>,
    frames: &mut Vec<FrameWait>,
) -> Option<Vec<(Path, u64)>> {
    let mut kept = Vec::with_capacity(records.len());
    for (path, id) in records {
        if held(id) {
            kept.push((path, id));
            continue;
        }
        let wait = super::layer_write::at(old, &path, id)
            .zip(super::layer_write::at(new, &path, id))
            .filter(|(a, _)| super::layer_band::records_band(a))
            .and_then(|(a, b)| waits(&path, a, b));
        let Some(wait) = wait else {
            crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
            return None;
        };
        frames.push(wait);
    }
    Some(kept)
}

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

fn grow(b: Bounds<ScaledPixels>, by: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: gpui::point(
            ScaledPixels(b.origin.x.0 - by),
            ScaledPixels(b.origin.y.0 - by),
        ),
        size: gpui::size(
            ScaledPixels(b.size.width.0 + 2. * by),
            ScaledPixels(b.size.height.0 + 2. * by),
        ),
    }
}

fn length(l: Option<Len>, none: f32) -> Option<f32> {
    match l {
        None => Some(none),
        Some(Len::Px(v)) => Some(v),
        Some(_) => None,
    }
}

/// How far beyond its box (or its fragments' bleed) the paint of `e` can
/// reach, CSS pixels: its outer shadows, text shadows and outline. `None`
/// for paint that reaches farther than this knows.
fn reach(e: &Element) -> Option<f32> {
    let c = BoxStyle::shared(e.style.used());
    let effects = &c.reset_effects;
    let border = &c.reset_border;
    if border.border_shape.is_some()
        || border.border_image.is_some()
        || effects.filter.is_some()
        || effects.filter_ref.is_some()
        || effects.drop_shadow.is_some()
        || effects.backdrop_blur.is_some()
        || effects.backdrop_color.is_some()
    {
        return None;
    }
    let ink = &c.inh.text_ink;
    let shadows = c
        .paint_shadows()
        .iter()
        .chain(ink.text_shadow.iter())
        .chain(ink.text_shadow_rest.iter());
    let mut out = 0f32;
    for s in shadows {
        out = out.max(s.x.abs().max(s.y.abs()) + 2. * s.blur.max(0.) + s.spread.max(0.));
    }
    if let Some(o) = &border.outline
        && o.style.is_some_and(|s| s != 0)
    {
        // `medium` is 3px; `auto` paints a focus ring beside the width.
        let width = length(o.width, 3.)?.max(0.);
        let offset = length(o.offset, 0.)?.max(0.);
        let auto = if o.style == Some(2) { width + 2. } else { 0. };
        out = out.max(width + offset + auto);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growing_keeps_the_centre() {
        let b = Bounds {
            origin: gpui::point(ScaledPixels(10.), ScaledPixels(10.)),
            size: gpui::size(ScaledPixels(4.), ScaledPixels(4.)),
        };
        let g = grow(b, 3.);
        assert_eq!(g.origin, gpui::point(ScaledPixels(7.), ScaledPixels(7.)));
        assert_eq!(g.size, gpui::size(ScaledPixels(10.), ScaledPixels(10.)));
    }

    #[test]
    fn a_length_other_than_pixels_is_unknown() {
        assert_eq!(length(None, 3.), Some(3.));
        assert_eq!(length(Some(Len::Px(2.)), 3.), Some(2.));
        assert_eq!(length(Some(Len::Pct(0.5)), 3.), None);
    }
}
