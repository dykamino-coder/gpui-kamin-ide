//! The retained layout result of a box (L1): a slot on the box tree record.
//!
//! Layout fills it; the box tree only keeps the slot and frees it
//! with the box.

/// The layout result of one box between frames.
#[derive(Debug, Default)]
pub(crate) struct LayoutSlot {}
