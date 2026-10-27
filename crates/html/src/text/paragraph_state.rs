//! Per-document measurement state of the text phase: shaped paragraph splits and text-fit
//! results (`MeasureCache`) and the previous-frame sizes of rotated vertical subtrees
//! (`VerticalMeasures`). The document owns one instance and hands it to its builds through
//! `LayoutCx::paragraph`; an embedded document has its own. Elements that read or record a
//! measurement after the build retain the exact shared handle, not the value context.

use crate::text::paragraph::MeasureCache;
use crate::text::vertical::VerticalMeasures;
use std::rc::Rc;

/// Paragraph and vertical-text measurements belonging to one document.
#[derive(Default)]
pub(crate) struct ParagraphState {
    /// Previous-frame sizes and per-frame occurrence keys of rotated subtrees.
    pub(crate) vertical: Rc<VerticalMeasures>,
    /// Shaped paragraphs and fit results shared only inside this document.
    pub(crate) paragraphs: Rc<MeasureCache>,
}

impl ParagraphState {
    /// Start of a document build: occurrence counters restart, recorded sizes stay
    /// as the next frame's fallback.
    pub(crate) fn begin_frame(&self) {
        self.vertical.begin_frame();
    }
}
