//! KaminIDE patch: the pieces of inline boxes' bands in a scene layer
//! (`text_system::band_piece`): their recording, and their recording again
//! from a new band style.

use super::*;
use crate::scene::{LayerHandle, LayerPainter, RunPainter};
use crate::text_system::band_piece::{BandEdges, paint_piece_style};
use crate::BandStyle;

impl Window {
    /// Whether a scene layer records the paint now.
    pub(crate) fn records_layer(&self) -> bool {
        self.layer_recording.is_some()
    }

    /// The paint `f` of a piece of the band of inline box `owner` on a line,
    /// segment `bounds`: in a layer recording the box's run, a recordable
    /// piece when `edges` are given.
    pub(crate) fn with_band_piece<R>(
        &mut self,
        owner: u32,
        bounds: Bounds<Pixels>,
        edges: Option<BandEdges>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.with_run(owner, bounds, RunPainter::Band(edges), f)
    }

    /// Box `owner` has paint in this recording outside its runs: the layer
    /// holds none of its runs and bindings (`SceneLayer::finish`).
    pub(crate) fn unhold_owner(&mut self, owner: u32) {
        if let Some(runs) = self.next_frame.scene.book.record_runs.as_mut() {
            runs.unheld.push(owner);
        } else if let Some(recording) = self.layer_recording.as_mut() {
            recording.unheld.push(owner);
        }
    }

    /// Paint every piece of the band of inline box `owner` in `layer` again
    /// with `style` and replace its runs with them, in the order of their
    /// lines' stacking layers (`Window::rerecord_layer_box`). `None`, the
    /// layer as it was for the runs not yet replaced, when a run of the box
    /// is no recordable piece (`LayerHandle::holds_band`).
    pub fn rerecord_layer_band(
        &mut self,
        layer: &LayerHandle,
        owner: u32,
        cx: &mut App,
        style: BandStyle,
    ) -> Option<bool> {
        self.rerecord_runs(layer, owner, cx, LayerPainter::Band, |window, _, env| {
            if let RunPainter::Band(Some(edges)) = env.painter {
                paint_piece_style(env.bounds, edges, style, window);
            }
        })
    }
}
