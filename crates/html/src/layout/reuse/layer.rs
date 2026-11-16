//! The document's scene layer between frames (`Store::layer`).
//!
//! The retained frame paints the document into a scene layer
//! (`gpui::Window::record_scene_layer`) and keeps it here. The paint ranges of
//! the block records made in that frame index the layer's scene, so the next
//! recording names it as its source, and a paint-only edit writes its values
//! into it (`document::revision::layer_write`) instead of a new frame.

use super::store::Store;
use std::cell::RefCell;
use std::collections::HashSet;

/// The document's scene layer of the last recorded frame, and the boxes whose
/// own commands the next frame records again into it (`queue_repaints`).
#[derive(Default)]
pub(crate) struct Kept {
    layer: RefCell<Option<gpui::LayerHandle>>,
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
    pub(crate) fn paint_layered(
        &self,
        window: &mut gpui::Window,
        f: impl FnOnce(&mut gpui::Window),
    ) {
        let source = self.scene_layer();
        // The frame painting now names its skipped blocks when it finishes.
        self.layer.unpainted.take();
        let shown = window.content_mask().bounds.scale(window.scale_factor());
        self.layer.deferred.start(Some(shown));
        let (_, layer) = window.record_scene_layer(source.as_ref(), f);
        let layer = match layer {
            gpui::RecordedLayer::Layer(layer) => {
                window.paint_scene_layer(&layer);
                Some(layer)
            }
            gpui::RecordedLayer::Spliced => {
                self.void_frame_records();
                None
            }
            gpui::RecordedLayer::Painted => None,
        };
        self.layer.layer.replace(layer);
    }

    /// `paint_layered` as the document's retained paint record.
    pub(crate) fn record_layered(
        &self,
        window: &mut gpui::Window,
        f: impl FnOnce(&mut gpui::Window),
    ) -> gpui::RetainedPaint {
        window
            .record_retained_paint(|window| self.paint_layered(window, f))
            .1
    }

    /// Drop the layer: the next frame paints a new one.
    pub(crate) fn drop_layer(&self) {
        self.layer.layer.replace(None);
        self.layer.deferred.start(None);
    }

    /// The layer of the last recorded frame, if it painted into one.
    pub(crate) fn scene_layer(&self) -> Option<gpui::LayerHandle> {
        self.layer.layer.borrow().clone()
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
