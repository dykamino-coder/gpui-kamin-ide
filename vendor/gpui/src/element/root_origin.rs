//! Absolute rounding origin for independently laid out element trees.

use super::AnyElement;
use crate::{App, AvailableSpace, Pixels, Point, Size, Window};

impl AnyElement {
    /// Prepare an independently cached measurement tree without laying out or painting it.
    pub fn snapshot_layout_measurement(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> crate::LayoutMeasurement {
        let root = self
            .0
            .root_layout_id()
            .unwrap_or_else(|| self.request_layout(window, cx));
        window.snapshot_layout_measurement(root)
    }

    /// Measure intrinsic contributions before device-pixel rounding. Adding
    /// rounded child widths makes content sizing depend on the display scale.
    pub fn layout_as_root_unrounded(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        self.layout_as_root(available_space, window, cx);
        let root = self.0.root_layout_id().expect("layout root was computed");
        window.layout_size_unrounded(root)
    }

    /// Like [`Self::layout_as_root_unrounded`], plus the extent of the
    /// root's visible content overflow (taffy `content_size`).
    pub fn layout_as_root_with_content(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> (Size<Pixels>, Size<Pixels>) {
        self.layout_as_root(available_space, window, cx);
        let root = self.0.root_layout_id().expect("layout root was computed");
        (
            window.layout_size_unrounded(root),
            window.layout_content_size_unrounded(root),
        )
    }

    /// Lay out a separate tree with its actual absolute origin. Its descendants
    /// then round shared device-pixel edges consistently with the surrounding
    /// tree. Prepaint at zero: the resulting layout bounds are absolute.
    ///
    /// Reuses an existing layout root, including one measured for intrinsic
    /// size, without requesting its layout state a second time.
    pub fn layout_as_root_at(
        &mut self,
        origin: Point<Pixels>,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        self.layout_as_root(available_space, window, cx);
        let root = self.0.root_layout_id().expect("layout root was computed");
        window.set_layout_root_origin(root, origin);
        window.layout_bounds(root).size
    }
}
