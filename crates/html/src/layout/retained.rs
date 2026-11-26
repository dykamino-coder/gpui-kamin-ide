//! The retained layout result of a box (L1): a slot on the box tree record.
//!
//! Layout fills it; the box tree only keeps the slot and frees it
//! with the box.

mod fragments;
mod ink;

pub(crate) use fragments::{FragmentTarget, InlineFragments};
pub(crate) use ink::measured;

/// The layout result of one box between frames.
#[derive(Debug, Default)]
pub(crate) struct LayoutSlot {
    /// The line fragments of the inline boxes of the paragraphs this box
    /// contains, as the last frame that painted them placed them
    /// (`fragments`); `None` until a paragraph of the box is painted.
    pub(crate) inline: Option<Box<InlineFragments>>,
    /// The ink overflow of the paragraphs of this box that place no
    /// fragments, as the frame of `.0` (`Store::epoch`) painted them, device
    /// pixels (`ink`); `.1` is `None` when one of them is unknown. `None`
    /// until such a paragraph is painted.
    pub(crate) ink: Option<(u64, Option<gpui::Bounds<gpui::ScaledPixels>>)>,
}
