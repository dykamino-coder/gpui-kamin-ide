//! Counters of the retained frame and of replayed blocks, read by tests and diagnostics.

use crate::Document;

impl Document {
    /// How many times the frame was built and how many times it was replayed (`render_viewport`).
    pub fn retained_frames(&self) -> (u64, u64) {
        (self.retained.builds.get(), self.retained.replays.get())
    }

    /// Blocks of the root flow: recorded, skipped off screen, and frames built
    /// again after a missed screen check (`layout::reuse`).
    pub fn reused_blocks(&self) -> (u64, u64, u64) {
        let stats = self.reuse.stats.get();
        (stats.recorded, stats.skipped, stats.rebuilt_frames)
    }

    /// Blocks wrapped for recording so far (`layout::reuse::recorder`).
    #[cfg(test)]
    pub(crate) fn tracked_blocks(&self) -> u64 {
        self.reuse.stats.get().tracked
    }

    /// Blocks of the root flow replayed on screen without a build.
    pub fn replayed_blocks(&self) -> u64 {
        self.reuse.stats.get().replayed
    }

    /// Blocks wrapped for a subtree replay record so far (`layout::reuse::replay`).
    #[cfg(test)]
    pub(crate) fn replay_tracked_blocks(&self) -> u64 {
        self.reuse.stats.get().replay_tracked
    }

    /// Of `replayed_blocks`, those whose layout subtree was claimed whole.
    #[cfg(test)]
    pub(crate) fn replayed_subtrees(&self) -> u64 {
        self.reuse.stats.get().subtrees
    }

    /// Why the frame was built instead of replayed since the last call: the
    /// counter name of each reason and its count (`record::FrameMiss`).
    pub fn take_frame_misses(&self) -> Vec<(String, u64)> {
        self.retained.take_misses()
    }

    /// The root scroller's largest offset (CSS px, positive down), as of its last layout.
    pub fn root_scroll_max(&self) -> Option<(f32, f32)> {
        let max = self.state.root_scroll.as_ref()?.handle.max_offset();
        Some((f32::from(max.x), f32::from(max.y)))
    }

    /// Open the root scroller at `(x, y)` CSS px from the top left (a fresh
    /// document compared with a scrolled one, engine-bench scroll mode).
    pub fn set_root_scroll_offset(&self, (x, y): (f32, f32)) {
        if let Some(root) = self.state.root_scroll.as_ref() {
            root.handle
                .set_offset(gpui::point(gpui::px(-x), gpui::px(-y)));
        }
    }

    /// Elements of box contents replayed at any depth (`layout::reuse::content`).
    pub fn replayed_contents(&self) -> u64 {
        self.reuse.stats.get().contents
    }

    /// Far blocks laid out from their kept layout subtree (`layout::reuse::far`).
    #[cfg(test)]
    pub(crate) fn far_blocks(&self) -> u64 {
        self.reuse.stats.get().far
    }

    /// Blocks far from the screen that stood as their leaf in painted frames
    /// (`layout::reuse::offscreen`).
    pub fn offscreen_leaves(&self) -> u64 {
        self.reuse.stats.get().offscreen
    }

    /// Builds of far blocks by the measures of their leaves in painted frames.
    pub fn offscreen_measures(&self) -> u64 {
        self.reuse.stats.get().offscreen_measures
    }

    /// Build this document's frames without leaves (`layout::reuse::offscreen`).
    #[cfg(test)]
    pub(crate) fn turn_off_offscreen_leaves(&self) {
        self.reuse.turn_off_offscreen_leaves();
    }

    /// Far records kept for the next build.
    #[cfg(test)]
    pub(crate) fn far_records(&self) -> usize {
        self.reuse.far_records()
    }
}
