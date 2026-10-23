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
}
