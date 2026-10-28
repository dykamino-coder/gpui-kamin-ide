//! Frame facts of a build next to the document's values (`FrameCx`): the line-measure owner
//! of the nearest fragmented ancestor (`LineCx`) and the build's memoized subtree and length
//! facts, the deferred paint owner of its render tree and its layout state. Layout and paint
//! code that reads these facts takes the frame context; calls into style, text and the value
//! arenas receive its `values` projection.

use crate::document::state::{ValueCx, ValueDoc};
use crate::layout::cx::deferred::Deferred;
use crate::layout::cx::subtree::SubtreeFacts;
use crate::layout::fragment::LineCx;
use crate::layout::fragment::line_shape::LengthFacts;
use crate::layout::layout_state::LayoutState;
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
    /// Deferred paint owner of the render tree being built.
    pub(crate) paint: Option<&'a Rc<Deferred>>,
    /// Layout state of the document (anchors under transforms).
    pub(crate) layout: &'a Rc<LayoutState>,
}

impl<'a> FrameCx<'a> {
    /// Values only: no fragmented ancestor, no memoized facts and no deferred paint owner.
    #[cfg(test)]
    pub(crate) fn new(values: ValueCx<'a>) -> Self {
        Self {
            values,
            lines: None,
            lengths: None,
            subtrees: None,
            paint: None,
            layout: &values.doc.layout,
        }
    }

    /// Retain the frame's values, deferred paint owner and layout state for effects built
    /// after the build (a positioned group's wrapper). Fragmentation and memoized facts
    /// belong to the build and are not retained.
    pub(crate) fn keep(self) -> FrameDoc {
        FrameDoc {
            values: self.values.keep(),
            paint: self.paint.cloned(),
            layout: self.layout.clone(),
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

/// Owned frame facts that outlive the build: values, deferred paint owner and layout state.
#[derive(Clone)]
pub(crate) struct FrameDoc {
    values: ValueDoc,
    paint: Option<Rc<Deferred>>,
    layout: Rc<LayoutState>,
}

impl FrameDoc {
    pub(crate) fn frame(&self) -> FrameCx<'_> {
        FrameCx {
            values: self.values.vcx(),
            lines: None,
            lengths: None,
            subtrees: None,
            paint: self.paint.as_ref(),
            layout: &self.layout,
        }
    }
}
