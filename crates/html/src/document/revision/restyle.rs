//! The block records over a rebuild of the same source DOM (`layout::reuse::Store::rebase`).

use crate::document::Document;

impl Document {
    /// `next` rebuilds this document's source DOM with the same node ids: it
    /// takes over the block records, compared now by the cascaded values of
    /// both trees (`Store::rebase`), so the previous frame tree goes with this
    /// document.
    pub(super) fn hand_over_reuse(&self, next: &mut Document) {
        self.reuse.rebase(&self.nodes, &next.nodes);
        next.reuse = self.reuse.clone();
    }
}
