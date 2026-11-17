//! Own-paint records of boxes off the screen, deferred until a frame shows
//! them (`Store::defer_repaints`).
//!
//! A paint-only edit of a box whose runs in the document's scene layer lie
//! wholly outside the area the layer's frame showed changes no pixel of that
//! frame. Its record waits here instead of being made now: the layer and its
//! damage stay as they are, so the frame needs neither a recording nor a
//! raster. A frame that replays the layer has the same window inputs
//! (`WindowKey`), so the box is still off the screen; it is recorded only
//! when a replay finds it in the area shown (`Store::due_repaints`). Any
//! other frame paints a new layer from the current boxes, whose records of
//! the edited blocks the restyle dropped (`Store::restyle`), and the deferred
//! records go with the old layer. Blink marks such an object for paint and
//! paints it only when the cull rect reaches it (`cull_rect_updater.cc`);
//! here the mark is a path, and no walk of either tree finds it.

use super::store::Store;
use gpui::{Bounds, LayerHandle, ScaledPixels};
use std::cell::Cell;

/// The area the layer's frame showed and the boxes whose own commands wait.
#[derive(Default)]
pub(crate) struct Deferred {
    /// The content mask of the frame that recorded the layer, device pixels.
    shown: Cell<Option<Bounds<ScaledPixels>>>,
    paths: Cell<Vec<(Vec<usize>, u64)>>,
}

/// Margin of the area shown for the rounding of edges, device pixels.
const SNAP: f32 = 2.0;

fn grown(b: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
    Bounds {
        origin: gpui::point(
            ScaledPixels(b.origin.x.0 - SNAP),
            ScaledPixels(b.origin.y.0 - SNAP),
        ),
        size: gpui::size(
            ScaledPixels(b.size.width.0 + 2.0 * SNAP),
            ScaledPixels(b.size.height.0 + 2.0 * SNAP),
        ),
    }
}

/// Whether box `owner`'s runs in `layer` miss `shown`.
fn misses(layer: &LayerHandle, owner: u64, shown: Bounds<ScaledPixels>) -> bool {
    u32::try_from(owner)
        .ok()
        .and_then(|o| layer.box_area(o))
        .is_some_and(|area| !grown(area).intersects(&shown))
}

impl Deferred {
    /// A new layer was recorded with the frame's content mask `shown`: it
    /// paints every box from its current style.
    pub(super) fn start(&self, shown: Option<Bounds<ScaledPixels>>) {
        self.shown.set(shown);
        self.paths.take();
    }
}

impl Store {
    /// Whether box `owner` of `layer` showed no pixel in the layer's frame:
    /// its record can wait (`defer_repaints`).
    pub(crate) fn off_screen(&self, layer: &LayerHandle, owner: u64) -> bool {
        let shown = self.layer.deferred().shown.get();
        shown.is_some_and(|shown| misses(layer, owner, shown))
    }

    /// Keep the boxes at `paths` off the screen to record when a replay
    /// shows them.
    pub(crate) fn defer_repaints(&self, paths: Vec<(Vec<usize>, u64)>) {
        if paths.is_empty() {
            return;
        }
        let deferred = self.layer.deferred();
        let mut kept = deferred.paths.take();
        kept.extend(paths);
        kept.sort_unstable_by(|a, b| (a.1, &a.0).cmp(&(b.1, &b.0)));
        kept.dedup();
        deferred.paths.set(kept);
    }

    /// The deferred boxes whose runs reach `shown`, the area a replay of the
    /// layer shows now; the rest keep waiting.
    pub(crate) fn due_repaints(
        &self,
        layer: &LayerHandle,
        shown: Bounds<ScaledPixels>,
    ) -> Vec<(Vec<usize>, u64)> {
        let deferred = self.layer.deferred();
        let kept = deferred.paths.take();
        if kept.is_empty() {
            return kept;
        }
        let (wait, due) = kept
            .into_iter()
            .partition(|(_, id)| misses(layer, *id, shown));
        deferred.paths.set(wait);
        due
    }

    /// The number of boxes whose records wait.
    #[cfg(test)]
    pub(crate) fn deferred_repaints(&self) -> usize {
        let paths = self.layer.deferred().paths.take();
        let n = paths.len();
        self.layer.deferred().paths.set(paths);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_margin_grows_both_sides() {
        let b = Bounds {
            origin: gpui::point(ScaledPixels(10.), ScaledPixels(20.)),
            size: gpui::size(ScaledPixels(5.), ScaledPixels(6.)),
        };
        let g = grown(b);
        assert_eq!(g.origin, gpui::point(ScaledPixels(8.), ScaledPixels(18.)));
        assert_eq!(g.size, gpui::size(ScaledPixels(9.), ScaledPixels(10.)));
    }
}
