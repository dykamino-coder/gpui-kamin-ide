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
//!   owner takes its owner's fragments, a box no band owner holds (a culled
//!   inline box gaining a band without padding or border) its container's
//!   lines; a box no fragment places (its paragraph was not painted in the
//!   layer's frame) everything below its nearest ancestor with a run in the
//!   layer (`below`);
//! - a box with decorations: its run in the layer, the box with its ink;
//! - both grown by the reach of the shadows and outline of either style; a
//!   filter, a border image or a border shape reaches farther than this
//!   knows, and is never deferred.

mod below;

use super::patch::{Path, chain};
use crate::dom::{Element, Node};
use crate::layout::reuse::{Store, Wait};
use crate::paint::decorations::outset;
use crate::style::computed::BoxStyle;
use gpui::{Bounds, LayerHandle, ScaledPixels};

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
    let Some(outset) = outset(&BoxStyle::shared(b.style.used())) else {
        crate::why::note("offscreen.unknown_area", || {
            format!("box {}: unbounded ink", a.node_id)
        });
        return None;
    };
    let area = match kind {
        Wait::Inline => inline_area(store, boxes, (old, path), a, b, outset).or_else(|| {
            let up = chain(old, &path[..path.len().saturating_sub(1)]);
            below::below(layer, &up, b, outset, store.shown_scale())
        }),
        Wait::Decoration => u32::try_from(a.node_id)
            .ok()
            .and_then(|owner| layer.decoration_area(owner, gpui::px(outset), store.shown_scale())),
    };
    let Some(area) = area else {
        crate::why::note("offscreen.unknown_area", || {
            let missing = match kind {
                Wait::Inline => unknown_fragments(store, boxes, a, || {
                    // The nearest block on the path: the container's element.
                    let parents = &path[..path.len().saturating_sub(1)];
                    let block = chain(old, parents).into_iter().rev().find(|e| !e.inline);
                    match block.and_then(|e| Some((e, u32::try_from(e.node_id).ok()?))) {
                        Some((e, owner)) => format!(
                            "block <{}> {owner}: runs {:?}, area {:?}, no run {:?}",
                            e.tag,
                            layer.run_painters(owner),
                            layer.box_area(owner),
                            layer.no_run(owner)
                        ),
                        None => "no block on the path".into(),
                    }
                }),
                Wait::Decoration => "no decorated run in the layer".into(),
            };
            format!("box {} <{}>: no {kind:?} area: {missing}", a.node_id, b.tag)
        });
        return None;
    };
    if !store.misses_shown(area) {
        crate::why::note("offscreen.on_screen", || {
            format!(
                "box {} <{}>: {kind:?} area {area:?} meets the shown area",
                a.node_id, b.tag
            )
        });
        return None;
    }
    Some((area, kind))
}

/// The fragments of inline box `a` (restyled to `b`) grown by `outset`,
/// device pixels. A box whose text no band owner holds (a culled inline box
/// and its culled ancestors) lies in its container's lines: their area bounds
/// its new band when the band adds no padding or border beyond the lines.
fn inline_area(
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    (old, path): (&[Node], &Path),
    a: &Element,
    b: &Element,
    outset: f32,
) -> Option<Bounds<ScaledPixels>> {
    let culled = || !super::layer_band::padded(b) && !super::layer_band::bordered(b);
    fragments_of(store, boxes, old, path, a, |f, owner| match owner {
        Some(owner) => f.ink(owner, outset),
        None if culled() => f.lines_ink(outset),
        None => None,
    })?
}

/// The container of inline box `id`'s line fragments: its nearest ancestor
/// whose slot holds fragments, or else whose inline content holds the box.
/// A paragraph writes into the box its flow belongs to
/// (`layout::reuse::fragment_target`), which may hold no published inline
/// content of its own (a paragraph of a render copy of the children).
fn container(
    boxes: &crate::document::boxes::BoxTree,
    id: crate::dom::BoxId,
) -> Option<crate::dom::BoxId> {
    boxes.ancestors(id).find(|p| {
        boxes.slot(*p).is_some_and(|s| s.inline.is_some())
            || boxes.get(*p).is_some_and(|r| r.inline.is_some())
    })
}

/// Why `fragments_of` knows no fragments of inline box `a` (DevTools `why`).
fn unknown_fragments(
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    a: &Element,
    container_run: impl FnOnce() -> String,
) -> String {
    let Some(id) = boxes.box_of(a.node_id) else {
        return "no box".into();
    };
    let Some(container) = container(boxes, id) else {
        return "no inline container".into();
    };
    match boxes.slot(container).map(|s| s.inline.as_deref()) {
        None => format!(
            "the container has no layout slot (its paragraphs wrote no fragments; {}): {}",
            container_run(),
            boxes.describe_chain(id, 6)
        ),
        Some(None) => "the container's paragraphs wrote no fragments".into(),
        Some(Some(f)) if !store.fragments_hold(f.stamp()) => {
            format!("fragments of frame {} do not describe the layer", f.stamp())
        }
        Some(Some(_)) => "no band owner and no lines hold the box".into(),
    }
}

/// `f` of the fragments of inline box `a` (or of the nearest band owner
/// holding it, `None` when no band owner does) in the slot of its container
/// and their owner, while they describe the layer.
fn fragments_of<R>(
    store: &Store,
    boxes: &crate::document::boxes::BoxTree,
    old: &[Node],
    path: &[usize],
    a: &Element,
    f: impl FnOnce(&crate::layout::retained::InlineFragments, Option<u32>) -> R,
) -> Option<R> {
    use crate::document::boxes::inline::ItemKind;
    let id = boxes.box_of(a.node_id)?;
    let container = container(boxes, id)?;
    let fragments = boxes.slot(container)?.inline.as_deref()?;
    if !store.fragments_hold(fragments.stamp()) {
        return None;
    }
    let own = u32::try_from(a.node_id).ok()?;
    if fragments.has(own) {
        return Some(f(fragments, Some(own)));
    }
    // The box's ancestors, outermost first (one descent along the path).
    let up = chain(old, &path[..path.len().saturating_sub(1)]);
    // A nested inline can inherit another run's owner. Only an opening item
    // in this recorded flow is covered by its enclosing inline fragments; a
    // container whose flow the box tree did not publish (a paragraph of a
    // render copy) covers a box reached from it through inline boxes alone.
    match boxes.get(container)?.inline.as_ref() {
        Some(content) => {
            let open = |item: &crate::document::boxes::inline::Item| {
                item.box_id == id && item.kind == ItemKind::Open
            };
            if !content.items().iter().any(open) {
                return None;
            }
        }
        None => {
            let inline = up
                .iter()
                .rev()
                .map_while(|e| (boxes.box_of(e.node_id)? != container).then_some(e.inline));
            if !inline.into_iter().all(|inline| inline) {
                return None;
            }
        }
    }
    let owner = up
        .iter()
        .rev()
        .map_while(|e| (boxes.box_of(e.node_id)? != container).then_some(e.node_id))
        .filter_map(|node| u32::try_from(node).ok())
        .find(|owner| fragments.has(*owner));
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
        let e = super::patch::element_at(&self.nodes, path)?;
        let boxes = self.state.boxes.borrow();
        fragments_of(&self.reuse, &boxes, &self.nodes, path, e, |f, owner| {
            owner.map(|owner| f.rects(owner).collect())
        })
        .flatten()
    }
}
