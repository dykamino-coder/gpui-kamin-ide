//! Text runs of a retained IFC in place of node-local normalization.
//!
//! The paragraph adapter still lowers its inline boxes (edges, bidi controls,
//! containing blocks); only the CSS Text preparation of each text node comes
//! from the box tree (`document::boxes::MixedText`), which normalized the flow
//! across its inline boundaries once. A text node takes its run only if it is
//! the node the box tree saw (same raw text, same position) under the inputs
//! the run read; otherwise the runs are dropped and the caller lowers the
//! paragraph as before.

use crate::document::boxes::MixedText;
use crate::document::boxes::inline::ItemKind;
use crate::style::computed::BoxStyle;

pub(crate) struct Runs {
    mixed: MixedText,
    /// Index of the next source item to match.
    at: usize,
    diverged: bool,
}

impl Runs {
    pub(crate) fn new(mixed: MixedText) -> Self {
        Self {
            mixed,
            at: 0,
            diverged: false,
        }
    }

    /// The prepared run of the next non-empty text node `raw` lowered with `style`.
    pub(crate) fn next(&mut self, raw: &str, style: &BoxStyle) -> Option<&str> {
        if self.diverged {
            return None;
        }
        let found = self.find(raw, style);
        if found.is_none() {
            self.diverged = true;
            return None;
        }
        found.map(|range| &self.mixed.text.text()[range])
    }

    fn find(&mut self, raw: &str, style: &BoxStyle) -> Option<std::ops::Range<usize>> {
        let source = &self.mixed.source;
        let offset = source.items()[self.at..]
            .iter()
            .position(|item| matches!(item.kind, ItemKind::Text { .. }))?;
        let index = self.at + offset;
        self.at = index + 1;
        let item = &source.items()[index];
        let range = item.kind.text_range()?;
        if source.text()[range] != *raw {
            return None;
        }
        // The owner's inputs were checked when the runs were handed out.
        if item.box_id != self.mixed.owner
            && !self
                .mixed
                .inputs
                .iter()
                .flat_map(|inputs| inputs.iter())
                .find(|(id, _)| *id == item.box_id)
                .is_some_and(|(_, inputs)| inputs.matches(style, style.tree.ruby_suppress_breaks))
        {
            return None;
        }
        self.mixed.text.items().get(index)?.kind.text_range()
    }

    /// Every text node took its run and no run is left over.
    pub(crate) fn complete(&self) -> bool {
        !self.diverged
            && !self.mixed.source.items()[self.at..]
                .iter()
                .any(|item| matches!(item.kind, ItemKind::Text { .. }))
    }
}
