//! The document's scene layer between frames (`Store::layer`).
//!
//! The retained frame paints the document into a scene layer
//! (`gpui::Window::record_scene_layer`) and keeps it here. The paint ranges of
//! the block records made in that frame index the layer's scene, so the next
//! recording names it as its source, and a paint-only edit writes its values
//! into it (`document::revision::layer_write`) instead of a new frame.

use super::store::Store;

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
    pub(crate) fn paint_layered(&self, window: &mut gpui::Window, f: impl FnOnce(&mut gpui::Window)) {
        let source = self.scene_layer();
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
        self.layer.replace(layer);
    }

    /// `paint_layered` as the document's retained paint record.
    pub(crate) fn record_layered(
        &self,
        window: &mut gpui::Window,
        f: impl FnOnce(&mut gpui::Window),
    ) -> gpui::RetainedPaint {
        window.record_retained_paint(|window| self.paint_layered(window, f)).1
    }

    /// The layer of the last recorded frame, if it painted into one.
    pub(crate) fn scene_layer(&self) -> Option<gpui::LayerHandle> {
        self.layer.borrow().clone()
    }
}
