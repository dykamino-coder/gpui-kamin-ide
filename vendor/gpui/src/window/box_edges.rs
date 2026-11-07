//! KaminIDE patch: read-only доступ фонового слоя к использованным кромкам.

use super::*;

impl Window {
    /// Used border and padding of the layer's parent, in logical pixels.
    pub fn parent_box_edges(&self, child: LayoutId) -> Option<(Edges<Pixels>, Edges<Pixels>)> {
        self.layout_engine
            .as_ref()
            .unwrap()
            .parent_box_edges(child, self.scale_factor())
    }

    /// Used border and padding of the box `id` itself, in logical pixels.
    pub fn box_edges(&self, id: LayoutId) -> Option<(Edges<Pixels>, Edges<Pixels>)> {
        self.layout_engine
            .as_ref()
            .unwrap()
            .box_edges(id, self.scale_factor())
    }

    /// Prepaint-time `(snapped, unrounded)` bounds of a would-be absolutely positioned child
    /// of `id` with these padding-box insets (see `TaffyLayoutEngine::inset_box`), offset
    /// like `layout_bounds` and `layout_origin_unrounded` offset that child.
    pub fn inset_layer_bounds(
        &mut self,
        id: LayoutId,
        insets: [f32; 4],
    ) -> Option<(Bounds<Pixels>, Bounds<Pixels>)> {
        let scale_factor = self.scale_factor();
        let (mut snapped, mut exact) =
            self.layout_engine
                .as_mut()
                .unwrap()
                .inset_box(id, insets, scale_factor)?;
        let offset = if self.exact_element_offset_depth > 0 {
            self.element_offset()
        } else {
            self.pixel_snap_point(self.element_offset())
        };
        snapped.origin += offset;
        exact.origin += self.element_offset();
        Some((snapped, exact))
    }
}
