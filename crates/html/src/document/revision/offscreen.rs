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
//! - both grown by shadow and outline reach; filters, border images and
//!   border shapes remain unknown and are never deferred.

use super::patch::{Path, element_at};
use crate::dom::{Element, Node};
use crate::layout::reuse::{Store, Wait};
use crate::paint::decorations::outset;
use crate::style::computed::BoxStyle;
use gpui::{Bounds, LayerHandle, ScaledPixels};

/// The area an edit off the screen can paint and why it waits for a frame.
pub(super) type FrameWait = (Bounds<ScaledPixels>, Wait);

/// Fragment geometry remains valid without a layer. Proven offscreen changes
/// to own paint preserve the frame; unknown bounds are rejected.
pub(super) fn without_layer(
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    old: &[Node],
    new: &[Node],
    paths: &[(Path, u64)],
) -> Option<Vec<FrameWait>> {
    use super::paint_damage::{PaintDamage, classify, unboxed, unpainted_svg};
    let mut frames = Vec::new();
    for (path, id) in paths {
        let (a, b) = (
            super::layer_write::at(old, path, *id)?,
            super::layer_write::at(new, path, *id)?,
        );
        let in_svg =
            (1..=path.len()).any(|k| element_at(old, &path[..k]).is_some_and(|e| e.tag == "svg"));
        // A leaf's text colour is deferred only here. Own-box layer painters
        // cannot rerecord its text runs when it becomes visible.
        if classify(a, b, || in_svg) == PaintDamage::Rebuild
            && (in_svg || super::paint_damage::leaf::color(a, b).is_none())
        {
            return declined(a, "not paint-only");
        }
        if unpainted_svg(old, path) || unboxed(a, b) {
            continue;
        }
        if store.skipped_any()
            && (1..=path.len())
                .any(|k| element_at(old, &path[..k]).is_some_and(|e| store.unpainted(e.node_id)))
        {
            crate::paint_path::add(&crate::paint_path::UNSEEN, 1);
            continue;
        }
        let Some(outset) = outset(&BoxStyle::shared(b.style.used())) else {
            return declined(a, "unknown paint outset");
        };
        let Some(area) = inline_area(store, boxes, old, path, a, outset) else {
            return declined(a, "no retained inline fragments");
        };
        if !store.misses_shown(area) {
            return declined(a, "paint reaches recorded area");
        }
        frames.push((area, Wait::Inline));
    }
    Some(frames)
}

/// Geometry refusals use the same observation protocol as layer refusals.
fn declined<T>(element: &Element, reason: &'static str) -> Option<T> {
    crate::why::note("offscreen.refused_without_layer", || {
        format!(
            "node={} tag={} reason={reason}",
            element.node_id, element.tag
        )
    });
    None
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
    kind: Wait,
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    layer: &LayerHandle,
    (old, path): (&[Node], &Path),
    a: &Element,
    b: &Element,
) -> Option<(Bounds<ScaledPixels>, Wait)> {
    // The old line bleed or decorated run belongs to the recorded frame.
    // Only the requested paint needs to be bounded here.
    let outset = outset(&BoxStyle::shared(b.style.used()))?;
    let area = match kind {
        Wait::Inline => inline_area(store, boxes, old, path, a, outset)?,
        Wait::Decoration => layer.decoration_area(
            u32::try_from(a.node_id).ok()?,
            gpui::px(outset),
            store.shown_scale(),
        )?,
    };
    store.misses_shown(area).then_some((area, kind))
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
    if let Some(area) = super::physical_paint::area(store, boxes, a, outset) {
        return Some(area);
    }
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
    use crate::document::boxes::inline::ItemKind;
    let id = boxes
        .box_of(a.node_id)
        .or_else(|| declined(a, "fragment box missing"))?;
    let container = boxes
        .ancestors(id)
        .find(|p| boxes.get(*p).is_some_and(|r| r.inline.is_some()))
        .or_else(|| declined(a, "inline container missing"))?;
    let fragments = boxes
        .slot(container)
        .and_then(|slot| slot.inline.as_deref())
        .or_else(|| {
            crate::why::note("offscreen.refused_without_layer", || {
                format!(
                    "node={} tag={} id={:?} class={:?} reason=container fragments missing {}",
                    a.node_id,
                    a.tag,
                    a.attr("id"),
                    a.attr("class"),
                    store.fragment_refusal(container)
                )
            });
            crate::why::note("offscreen.fragment_ancestors", || {
                (1..path.len())
                    .filter_map(|n| element_at(old, &path[..n]))
                    .map(|e| {
                        let id = boxes.box_of(e.node_id);
                        let geometry = id
                            .and_then(|id| boxes.slot(id))
                            .and_then(|slot| slot.inline.as_deref())
                            .map(|f| {
                                (
                                    f.stamp(),
                                    u32::try_from(a.node_id).ok().is_some_and(|id| f.has(id)),
                                )
                            });
                        format!(
                            "node={} tag={} id={:?} class={:?} box={id:?} display={:?} \
                             geometry={geometry:?} capture={}",
                            e.node_id,
                            e.tag,
                            e.attr("id"),
                            e.attr("class"),
                            e.style.used().reset_box_layout.display,
                            id.map(|id| store.fragment_refusal(id)).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            });
            None
        })?;
    if !store.fragments_hold(fragments.stamp()) {
        return declined(a, "container fragments do not describe recorded frame");
    }
    let own = u32::try_from(a.node_id).ok()?;
    if fragments.has(own) {
        return Some(f(fragments, own));
    }
    // A nested inline can inherit another run's owner. Only an opening item
    // in this recorded flow is covered by its enclosing inline fragments.
    let content = boxes.get(container)?.inline.as_ref()?;
    if !content
        .items()
        .iter()
        .any(|item| item.box_id == id && item.kind == ItemKind::Open)
    {
        return declined(a, "inline opening item missing");
    }
    let owner = (1..path.len())
        .rev()
        .map_while(|n| {
            let e = element_at(old, &path[..n])?;
            (boxes.box_of(e.node_id)? != container).then_some(e.node_id)
        })
        .filter_map(|node| u32::try_from(node).ok())
        .find(|owner| fragments.has(*owner))
        .or_else(|| declined(a, "inline fragment owner missing"))?;
    Some(f(fragments, owner))
}

impl crate::document::Document {
    /// The line boxes of the fragments of the inline box at `path`, window
    /// pixels, as the recorded frame placed them (`getClientRects()`). A
    /// synchronous reader flushes layout when no frame describes them now.
    #[cfg_attr(not(test), allow(dead_code, reason = "read by the script binding"))]
    pub(crate) fn inline_rects(&self, path: &[usize]) -> Option<Vec<gpui::Bounds<gpui::Pixels>>> {
        let e = element_at(&self.nodes, path)?;
        let boxes = self.state.boxes.borrow();
        fragments_of(&self.reuse, &boxes, &self.nodes, path, e, |f, owner| {
            f.rects(owner).collect()
        })
    }
}
