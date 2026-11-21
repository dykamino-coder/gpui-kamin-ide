//! Counters observing block replay.

/// Counters observing block replay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReuseStats {
    /// Blocks wrapped for recording (`recorder::Making`).
    pub tracked: u64,
    pub recorded: u64,
    pub skipped: u64,
    pub rebuilt_frames: u64,
    /// Blocks of the root flow replayed on screen without a build: skipped
    /// blocks whose output of the last frame was replayed, and blocks whose
    /// layout subtree was claimed whole (`replay`).
    pub replayed: u64,
    /// Of `replayed`, blocks whose layout subtree was claimed whole.
    pub subtrees: u64,
    /// Blocks wrapped for a subtree replay record (`replay::PendingReplay`).
    pub replay_tracked: u64,
    /// Far blocks laid out from their kept layout subtree without a build
    /// (`far`).
    pub far: u64,
    /// Elements of box contents replayed at any depth (`content`), apart
    /// from the root flow's blocks.
    pub contents: u64,
    /// Blocks far from the screen that stood as their leaf in a painted
    /// frame (`offscreen`).
    pub offscreen: u64,
    /// Builds of far blocks by the measures of their leaves (`offscreen`):
    /// more than one per leaf is a block built at more than one width.
    pub offscreen_measures: u64,
}
