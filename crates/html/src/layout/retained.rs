//! The retained layout result of a box (L1): a slot on the box tree record.
//!
//! Layout fills it; the box tree only keeps the slot and frees it
//! with the box.

mod fragments;
mod paint_bounds;

pub(crate) use fragments::{FragmentTarget, InlineFragments};
pub(crate) use paint_bounds::PaintBounds;
pub(crate) use paint_bounds::finite as finite_paint_bounds;

/// The layout result of one box between frames.
#[derive(Debug, Default)]
pub(crate) struct LayoutSlot {
    /// The line fragments of the inline boxes of the paragraphs this box
    /// contains, as the last frame that painted them placed them
    /// (`fragments`); `None` until a paragraph of the box is painted.
    pub(crate) inline: Option<Box<InlineFragments>>,
    /// Physical text pieces: conservative paint damage, never CSSOM geometry.
    pub(crate) text_paint: Option<Box<PaintBounds>>,
}
