//! Counters of the retained frame and of replayed blocks, read by tests and diagnostics.

use crate::Document;

impl Document {
    /// Сколько раз кадр собран и сколько раз повторён (`render_viewport`).
    pub fn retained_frames(&self) -> (u64, u64) {
        (self.retained.builds.get(), self.retained.replays.get())
    }

    /// Блоки корневого потока: записано, пропущено вне экрана, кадров
    /// собрано заново после промаха проверки экрана (`layout::reuse`).
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

    /// Elements of box contents replayed at any depth (`layout::reuse::content`).
    pub fn replayed_contents(&self) -> u64 {
        self.reuse.stats.get().contents
    }

    /// Far blocks laid out from their kept layout subtree (`layout::reuse::far`).
    #[cfg(test)]
    pub(crate) fn far_blocks(&self) -> u64 {
        self.reuse.stats.get().far
    }

    /// Far records kept for the next build.
    #[cfg(test)]
    pub(crate) fn far_records(&self) -> usize {
        self.reuse.far_records()
    }
}
