//! The document's scene layer between frames (`Store::layer`).
//!
//! The retained frame paints the document into a scene layer
//! (`gpui::Window::record_scene_layer`) and keeps it here. The paint ranges of
//! the block records made in that frame index the layer's scene, so the next
//! recording names it as its source, and a paint-only edit writes its values
//! into it (`document::revision::layer_write`) instead of a new frame.

use super::store::Store;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

/// The document's scene layer of the last recorded frame, and the boxes whose
/// own commands the next frame records again into it (`queue_repaints`).
#[derive(Default)]
pub(crate) struct Kept {
    /// Inclusive run of frames that hold the same block ranges. A rebuild,
    /// a gap or an unverified copy ends the run without walking any records.
    copied: Cell<Option<(u64, u64)>>,
    layer: RefCell<Option<gpui::LayerHandle>>,
    /// Why `layer` is empty: the last recording's refusal or the drop (DevTools `why`).
    no_layer: Cell<Option<gpui::NoLayer>>,
    repaints: RefCell<Vec<(Vec<usize>, u64)>>,
    /// The root-flow blocks the layer's frame skipped off the screen
    /// (`FlowReuse::finish`): no box of theirs is in the layer.
    unpainted: RefCell<HashSet<u64>>,
    /// Records of boxes off the screen, waiting for a frame that shows them.
    deferred: super::deferred::Deferred,
}

impl Kept {
    pub(super) fn deferred(&self) -> &super::deferred::Deferred {
        &self.deferred
    }
}

impl Store {
    /// The whole document was copied to the same frame slots and its scene
    /// layer kept its version. Only inline, complete block output can carry.
    pub(crate) fn copied_layer_ranges(&self) {
        let frame = self.epoch.get();
        let previous = frame - 1;
        let first = self
            .layer
            .copied
            .get()
            .filter(|&(_, last)| last == previous)
            .map_or(previous, |(first, _)| first);
        self.layer.copied.set(Some((first, frame)));
    }

    pub(super) fn block_source(&self, frame: u64, recorded: u64) -> bool {
        self.layer
            .copied
            .get()
            .is_some_and(|(first, last)| last == frame && recorded >= first && recorded < frame)
    }

    /// The paint of the frame being drawn did not stay at its recorded
    /// indices (`gpui::RecordedLayer::Spliced`): no record made in it
    /// replays, as when the chain of frames breaks (`draw`).
    pub(crate) fn void_frame_records(&self) {
        self.forget_replays();
        self.epoch.set(self.epoch.get() + 1);
    }

    /// Paint the document `f` into a new scene layer drawn here, with the
    /// last one as the source of the blocks replayed inside, and keep it.
    /// Paint the layer cannot hold goes into the frame (`RecordedLayer`).
    ///
    /// Returns the record of `f` inside the layer (its ranges index the
    /// layer's scene), `None` when the content did not land in a layer.
    /// `fresh`: `f` paints the tree (not a copy of the last layer's commands,
    /// which keeps the blocks it skipped and its deferred records).
    pub(crate) fn paint_layered(
        &self,
        window: &mut gpui::Window,
        fresh: bool,
        f: impl FnOnce(&mut gpui::Window),
    ) -> Option<gpui::RetainedPaint> {
        let source = self.scene_layer();
        if fresh {
            // The frame painting now names its skipped blocks when it finishes.
            self.layer.unpainted.take();
            let shown = window.content_mask().bounds.scale(window.scale_factor());
            self.layer
                .deferred
                .start(Some(shown), self.epoch.get(), window.scale_factor());
        }
        let mut inner = None;
        let (_, layer) = window.record_scene_layer(source.as_ref(), |window| {
            inner = Some(window.record_retained_paint(f).1);
        });
        let (layer, why) = match layer {
            gpui::RecordedLayer::Layer(layer) => {
                window.paint_scene_layer(&layer);
                (Some(layer), None)
            }
            gpui::RecordedLayer::Spliced(why) => {
                self.void_frame_records();
                (None, Some(why))
            }
            gpui::RecordedLayer::Painted(why) => (None, Some(why)),
        };
        self.layer.no_layer.set(why);
        let inner = inner.filter(|_| layer.is_some());
        self.layer.layer.replace(layer);
        inner
    }

    /// `paint_layered` as the document's retained paint record: the record
    /// in the frame and the one inside the layer.
    pub(crate) fn record_layered(
        &self,
        window: &mut gpui::Window,
        fresh: bool,
        f: impl FnOnce(&mut gpui::Window),
    ) -> (gpui::RetainedPaint, Option<gpui::RetainedPaint>) {
        let (inner, outer) =
            window.record_retained_paint(|window| self.paint_layered(window, fresh, f));
        (outer, inner)
    }

    /// Drop the layer: the next frame paints a new one.
    #[track_caller]
    pub(crate) fn drop_layer(&self) {
        self.layer.layer.replace(None);
        let site = std::panic::Location::caller();
        let why = gpui::NoLayer { reason: "dropped", site, groups: 0, backdrops: 0 };
        self.layer.no_layer.set(Some(why));
        self.layer.deferred.start(None, self.epoch.get(), 1.);
    }

    /// The layer of the last recorded frame, if it painted into one.
    pub(crate) fn scene_layer(&self) -> Option<gpui::LayerHandle> {
        self.layer.layer.borrow().clone()
    }

    /// Why there is no `scene_layer`: `None` when none was recorded yet.
    pub(crate) fn no_layer(&self) -> Option<gpui::NoLayer> {
        self.layer.no_layer.get()
    }

    /// Identity and content version of the layer containing the block ranges.
    pub(crate) fn layer_version(&self) -> Option<(u64, u64)> {
        let layer = self.layer.layer.borrow();
        let content = layer.as_ref()?.content();
        Some((content.id(), content.version()))
    }

    /// Queue the boxes at `paths` (child indices from the root, with their
    /// box numbers) to record their own commands again into the layer before
    /// the next frame replays (`document::retained`). The edits that queue
    /// them keep the tree's shape, so a path addresses its box until the next
    /// frame; the replay checks the box number at it.
    pub(crate) fn queue_repaints(&self, paths: Vec<(Vec<usize>, u64)>) {
        let mut queued = self.layer.repaints.borrow_mut();
        queued.extend(paths);
        // Fragments of one element are adjacent: they repaint together.
        queued.sort_unstable_by(|a, b| (a.1, &a.0).cmp(&(b.1, &b.0)));
        queued.dedup();
    }

    /// Take the queued boxes, grouped by box number.
    pub(crate) fn take_repaints(&self) -> Vec<(Vec<usize>, u64)> {
        std::mem::take(&mut *self.layer.repaints.borrow_mut())
    }

    /// The blocks the frame of the layer skipped off the screen, even with
    /// their boxes repainted (`FlowReuse::validate`).
    pub(crate) fn set_unpainted(&self, blocks: HashSet<u64>) {
        self.layer.unpainted.replace(blocks);
    }

    /// Whether the layer's frame skipped block `id` off the screen: a
    /// paint-only change of a box in it shows no pixel there. The block's
    /// record takes the change (`Store::restyle`): a frame that brings it
    /// near the screen builds or validates it with its new paint, as the
    /// edit of a skipped block in Blink only marks it for paint when the cull
    /// rect reaches it (`cull_rect_updater.cc`).
    pub(crate) fn unpainted(&self, id: u64) -> bool {
        self.layer.unpainted.borrow().contains(&id)
    }

    /// Whether the layer's frame skipped any block.
    pub(crate) fn skipped_any(&self) -> bool {
        !self.layer.unpainted.borrow().is_empty()
    }
}
