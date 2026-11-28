//! Bench modes distinguish complete trajectories from single-frame samples.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Mode {
    Cold,
    Warm,
    Both,
    Relayout,
    /// One wheel step on the root scroller before each frame (`frames::WHEEL_STEP`).
    Scroll,
    /// A complete live-vsync trajectory, with no single-frame timing claim.
    Smooth,
    /// Local markup edits between frames (`edit::Edit`, `edits`).
    Edit,
}

#[cfg(windows)]
impl Mode {
    pub(crate) fn phases(self) -> &'static [(&'static str, bool)] {
        match self {
            Self::Cold => &[("cold", true)],
            Self::Warm => &[("warm", false)],
            Self::Both => &[("cold", true), ("warm", false)],
            Self::Relayout => &[("relayout", false)],
            Self::Scroll => &[("scroll", false)],
            Self::Edit | Self::Smooth => &[],
        }
    }
}
