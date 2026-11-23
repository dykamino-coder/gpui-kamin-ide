//! Boxes whose own paint a restyle changed, recorded again into the document's
//! scene layer before the recorded frame replays (`Store::take_repaints`).
//!
//! The revision kept the frame (`revision::layer_write`); each queued box
//! paints its own commands again from its new style into its run of the
//! layer (`gpui::Window::rerecord_layer_box`), without its descendants,
//! layout or the element tree. Its style comes from the one `Div` the box
//! would build, without its children (`paint::box_tree::styled::own_paint_div`),
//! the interim painter until the boxes own their used style (B1). A box with
//! decorations (child elements) that is all its composition group holds
//! paints the group again: its own paint and its decorations without layout
//! (`styled::group_painters`), the group's scene replaced whole
//! (`gpui::Window::rerecord_layer_group`). A box the layer refuses voids the
//! replay and the layer: the frame is built and paints a new one.

use super::Builder;
use gpui::{App, Refineable, Styled, Window};

/// Paint box `owner`, all its group holds, again with its decorations
/// (`gpui::Window::rerecord_layer_group`): its own paint, then each
/// decoration in the box layout gives it from the padding box, as the box's
/// child elements paint them. `None`, the layer unchanged, when the group
/// cannot take it, or layout would place a decoration otherwise: the box's
/// recording knew no unrounded box equal to its snapped one, a decoration's
/// box is off the device grid, or the box clips its children. The error
/// names the reason for the DevTools `why` journal.
fn group_paint(
    window: &mut Window,
    cx: &mut App,
    layer: &gpui::LayerHandle,
    owner: u32,
    style: &gpui::Style,
    mut painters: crate::paint::decorations::Painters,
) -> Result<(), &'static str> {
    let mut why =
        "the layer refused the group's new paint (no group root, or it does not fit the group)";
    let painted = window.rerecord_layer_group(layer, owner, cx, |window, cx, bounds, exact| {
        if !painters.is_empty() {
            if exact != Some(bounds) {
                why = "the recording knew no unrounded box equal to the snapped one";
                return None;
            }
            if style.overflow_mask_snapped(bounds, window).is_some() {
                why = "the box clips its decorations";
                return None;
            }
        }
        style.paint_box(bounds, window, cx);
        if painters.is_empty() {
            return Some(());
        }
        let edges = style.border_widths.to_pixels(window.rem_size());
        let padding = gpui::Bounds::from_corners(
            gpui::point(bounds.left() + edges.left, bounds.top() + edges.top),
            gpui::point(bounds.right() - edges.right, bounds.bottom() - edges.bottom),
        );
        let painted = painters.paint(padding, window.scale_factor(), window);
        if painted.is_none() {
            why = "a decoration's box is off the device grid";
        }
        painted
    });
    painted.ok_or(why)
}

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
            // An edit the layer cannot record that this replay shows: one
            // frame is built for every edit waiting (`layout::reuse::Wait`).
            if self.store.due_frames(shown) {
                return false;
            }
            self.store
                .queue_repaints(self.store.due_repaints(&layer, shown));
        }
        let ids = self.store.take_repaints();
        if ids.is_empty() {
            return true;
        }
        let mut moved = false;
        let mut last = None;
        for (path, id) in &ids {
            let Some(layer) = self.store.owner_layer(*id) else {
                return false;
            };
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
            if layer.painter(owner) == Some(gpui::LayerPainter::Band) {
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
            // A box that is all its group holds paints the group again with
            // its decorations (`group_paint`): its run may also hold the
            // decorations an earlier group paint put there, whatever painter
            // first recorded it. A box with decorations, before or now,
            // paints no other way.
            let decorated = layer.painter(owner) == Some(gpui::LayerPainter::DecoratedBox);
            let painters = crate::paint::box_tree::styled::group_painters(e, &self.opts.cx);
            let grouped = layer.holds_group_root(owner) && painters.is_some();
            if grouped || decorated || painters.as_ref().is_some_and(|p| !p.is_empty()) {
                let painted = match painters {
                    Some(painters) if layer.holds_group_root(owner) => {
                        group_paint(window, cx, &layer, owner, &style, painters)
                    }
                    Some(_) => Err("a decorated box that is not all its group holds"),
                    None => Err("its decorations need layout (no group painters)"),
                };
                if let Err(why) = painted {
                    crate::paint_path::refuse_with(&crate::paint_path::REFUSED_BY_LAYER, || {
                        format!("group repaint of box {owner}: {why}")
                    });
                    return false;
                }
                continue;
            }
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
    ) -> (gpui::RetainedPaint, Option<gpui::RetainedPaint>) {
        self.store.take_repaints();
        self.store.record_layered(window, true, f)
    }

    /// A scrolled replay of the document's last layer (`inner`, its record
    /// there) painted into a new layer: the commands move under their
    /// scroll nodes (`gpui::Window::replay_retained_paint_scrolled`).
    pub(super) fn replay_paint_scrolled(
        &self,
        window: &mut Window,
        inner: &gpui::RetainedPaint,
        plan: &gpui::ScrollReplay,
    ) -> (gpui::RetainedPaint, Option<gpui::RetainedPaint>) {
        self.store.record_layered(window, false, |window| {
            window.replay_retained_paint_scrolled(inner, plan);
        })
    }
}
