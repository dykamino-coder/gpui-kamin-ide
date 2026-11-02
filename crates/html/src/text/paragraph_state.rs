//! Per-document measurement state of the text phase: shaped paragraph splits and text-fit
//! results (`MeasureCache`) and the previous-frame sizes of rotated vertical subtrees
//! (`VerticalMeasures`). The document owns one instance and hands it to its builds through
//! `LayoutCx::paragraph`; an embedded document has its own. Elements that read or record a
//! measurement after the build retain the exact shared handle, not the value context.

use crate::text::paragraph::MeasureCache;
use crate::text::vertical::VerticalMeasures;
use std::cell::Cell;
use std::rc::Rc;

/// Paragraph and vertical-text measurements belonging to one document.
#[derive(Default)]
pub(crate) struct ParagraphState {
    /// Previous-frame sizes and per-frame occurrence keys of rotated subtrees.
    pub(crate) vertical: Rc<VerticalMeasures>,
    /// Shaped paragraphs and fit results shared only inside this document.
    pub(crate) paragraphs: Rc<MeasureCache>,
    /// The host began a frame (`DocState::begin_frame`) and the tree build has not yet
    /// sanitized it: that build must not rotate the measurement cache a second time.
    begun: Cell<bool>,
}

impl ParagraphState {
    /// Measurements of the next revision of the same document: the value-keyed paragraph
    /// cache carries over; rotated-subtree sizes are facts of the previous tree and do not.
    pub(crate) fn continuing(previous: &Self) -> Self {
        Self {
            vertical: Default::default(),
            paragraphs: previous.paragraphs.clone(),
            begun: Cell::new(false),
        }
    }

    /// Start of a frame by the host: occurrence counters restart, recorded sizes stay
    /// as the next frame's fallback, and the measurement cache keeps exactly the
    /// previous frame's entries.
    pub(crate) fn begin_frame(&self) {
        self.vertical.begin_frame();
        self.paragraphs.begin_frame();
        self.begun.set(true);
    }

    /// Start of a tree build (`interactive::frame::frame_sanitize`). Occurrence counters
    /// restart for every build; the measurement cache rotates once per frame: a build
    /// right after the host's `begin_frame` keeps the entries that frame just carried
    /// over, a build without one (raw `render` of a parsed tree) starts its own frame.
    pub(crate) fn begin_build(&self) {
        self.vertical.begin_frame();
        if !self.begun.replace(false) {
            self.paragraphs.begin_frame();
        }
    }
}
