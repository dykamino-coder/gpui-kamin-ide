//! Frame facts of a build next to the document's values (`FrameCx`): the line-measure owner
//! of the nearest fragmented ancestor (`LineCx`) and the build's memoized subtree and length
//! facts. Layout algorithms that read these facts take the frame context; calls into style,
//! text and the value arenas receive its `values` projection.

use crate::document::state::ValueCx;
use crate::layout::cx::subtree::SubtreeFacts;
use crate::layout::fragment::LineCx;
use crate::layout::fragment::line_shape::LengthFacts;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy)]
pub(crate) struct FrameCx<'a> {
    /// The document's values (arenas, viewport, scheme).
    pub(crate) values: ValueCx<'a>,
    /// Line-measure inputs of the nearest fragmented ancestor, if any.
    pub(crate) lines: Option<&'a Rc<LineCx>>,
    /// Lengths already resolved for unchanged subtrees of this build.
    pub(crate) lengths: Option<&'a Rc<RefCell<LengthFacts>>>,
    /// Break and float facts already proven absent for subtrees of this build.
    pub(crate) subtrees: Option<&'a Rc<RefCell<SubtreeFacts>>>,
}

impl<'a> FrameCx<'a> {
    /// Values only: no fragmented ancestor and no memoized facts.
    #[cfg(test)]
    pub(crate) fn new(values: ValueCx<'a>) -> Self {
        Self {
            values,
            lines: None,
            lengths: None,
            subtrees: None,
        }
    }

    /// The same frame under another line-measure owner.
    pub(crate) fn with_lines(mut self, lines: Option<&'a Rc<LineCx>>) -> Self {
        self.lines = lines;
        self
    }

    /// The same frame with memoized length facts.
    #[cfg(test)]
    pub(crate) fn with_lengths(mut self, facts: &'a Rc<RefCell<LengthFacts>>) -> Self {
        self.lengths = Some(facts);
        self
    }
}

impl std::fmt::Debug for FrameCx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameCx")
            .field("values", &self.values)
            .field("lines", &self.lines.is_some())
            .finish()
    }
}
