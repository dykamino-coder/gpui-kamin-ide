//! KaminIDE patch: a smaller layout pass releases tables left by a larger document.

use super::TaffyLayoutEngine;

impl TaffyLayoutEngine {
    pub(super) fn clear_pass_storage(&mut self) {
        // Shrink while the completed pass's entries still exist. Clearing first
        // would free every table on every frame, even for an unchanged document.
        // An unchanged working set keeps its allocation; a smaller one releases
        // the buckets used only by an earlier pass.
        self.absolute_unrounded.shrink_to_fit();
        self.absolute_layout_bounds.shrink_to_fit();
        self.root_origins.shrink_to_fit();
        self.placed_origins.shrink_to_fit();
        self.computed_layouts.shrink_to_fit();
        self.absolute_unrounded.clear();
        self.absolute_layout_bounds.clear();
        self.root_origins.clear();
        self.placed_origins.clear();
        self.computed_layouts.clear();
    }
}
