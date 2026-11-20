//! The retained layout result of a box (L1): a slot on the box tree record.
//!
//! Layout fills it; the box tree only keeps the slot and frees it
//! with the box.

mod fragments;

pub(crate) use fragments::{FragmentTarget, InlineFragments};

/// The layout result of one box between frames.
#[derive(Debug, Default)]
pub(crate) struct LayoutSlot {
    /// The line fragments of the inline boxes of the paragraphs this box
    /// contains, as the last frame that painted them placed them
    /// (`fragments`); `None` until a paragraph of the box is painted.
    pub(crate) inline: Option<Box<InlineFragments>>,
}
