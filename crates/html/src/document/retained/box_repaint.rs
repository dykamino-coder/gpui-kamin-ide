//! Boxes whose own paint a restyle changed, recorded again into the document's
//! scene layer before the recorded frame replays (`Store::take_repaints`).
//!
//! The revision kept the frame (`revision::layer_write`); each queued box
//! paints its own commands again from its new style into its run of the
//! layer (`gpui::Window::rerecord_layer_box`), without its descendants,
//! layout or the element tree. Its style comes from the one `Div` the box
//! would build, without its children (`paint::box_tree::styled::own_paint_div`),
//! the interim painter until the boxes own their used style (B1). A box the layer refuses voids the
//! replay and the layer: the frame is built and paints a new one.

use super::Builder;
use gpui::{App, Refineable, Styled, Window};

impl Builder {
    /// Record the queued boxes again before the frame replays; `false` when
    /// the frame must be built.
    pub(super) fn repaint_boxes(&self, window: &mut Window, cx: &mut App) -> bool {
        let done = self.repaint_queued(window, cx);
        if !done {
            // Runs already replaced leave the layer neither old nor new.
            self.store.void_frame_records();
            self.store.drop_layer();
        }
        done
    }

    fn repaint_queued(&self, window: &mut Window, cx: &mut App) -> bool {
        // Deferred boxes this replay shows are recorded with the queued ones.
        if let Some(layer) = self.store.scene_layer() {
            let shown = window.content_mask().bounds.scale(window.scale_factor());
            self.store
                .queue_repaints(self.store.due_repaints(&layer, shown));
        }
        let ids = self.store.take_repaints();
        if ids.is_empty() {
            return true;
        }
        let Some(layer) = self.store.scene_layer() else {
            return false;
        };
        let mut moved = false;
        let mut last = None;
        for (path, id) in &ids {
            // The box at its path, without a walk of the tree.
            let Some(e) = crate::document::revision::at(&self.nodes, path, *id) else {
                return false;
            };
            // Fragments of one element share an owner and are repainted together.
            if last == Some(e.node_id) {
                continue;
            }
            last = Some(e.node_id);
            let Ok(owner) = u32::try_from(e.node_id) else {
                return false;
            };
            // An inline box: the pieces of its band, from its new band.
            if e.inline && e.style.used().reset_box_layout.display.is_none() {
                let Some(parent) = crate::document::revision::parent_at(&self.nodes, path) else {
                    return false;
                };
                let band = crate::text::inline::band_of(e, parent, self.opts.cx.vcx());
                match window.rerecord_layer_band(&layer, owner, cx, band) {
                    Some(shifted) => moved |= shifted,
                    None => {
                        crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
                        return false;
                    }
                }
                continue;
            }
            let mut div = crate::paint::box_tree::styled::own_paint_div(e, &self.opts.cx);
            let mut style = gpui::Style::default();
            style.refine(div.style());
            let painted = window.rerecord_layer_box(&layer, owner, cx, |window, cx, bounds| {
                style.paint_box(bounds, window, cx)
            });
            match painted {
                Some(shifted) => moved |= shifted,
                None => {
                    crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
                    return false;
                }
            }
        }
        if moved {
            // Ranges recorded in the layer moved with the new commands.
            self.store.void_frame_records();
        }
        true
    }

    /// The document's paint, recorded into its scene layer; a frame that
    /// builds paints every box anew, so no queued box is recorded again.
    pub(super) fn record_paint(
        &self,
        window: &mut Window,
        f: impl FnOnce(&mut Window),
    ) -> gpui::RetainedPaint {
        self.store.take_repaints();
        self.store.record_layered(window, f)
    }
}
