//! The block records over a rebuild of the same source DOM (`layout::reuse::Store::rebase`).

use super::mutation::{Batch, attribute_value, open};
use crate::document::Document;
use crate::dom::invalidation::Changed;

impl Document {
    /// `next` rebuilds this document's source DOM with the same node ids: it
    /// takes over the block records, compared now by the cascaded values of
    /// the elements the rebuild restyled (`restyled`, `dom::invalidation`)
    /// or, if those are unknown, of both trees (`Store::rebase`), so the
    /// previous frame tree goes with this document.
    pub(super) fn hand_over_reuse(&self, next: &mut Document, restyled: Option<&[u64]>) {
        crate::document::boxes::BoxTree::edit(&next.state.boxes).publish_document(
            &next.nodes,
            Some(&self.nodes),
            &next.state.values,
        );
        self.reuse.rebase(&self.nodes, &next.nodes, restyled);
        next.reuse = self.reuse.clone();
    }

    /// The names the attribute mutations of `batch` change, for the rebuild
    /// of this document's source DOM under the same theme to find the
    /// elements they restyle (`dom::invalidation`); `None`: the batch changes
    /// texts or attributes that the engine reads outside selectors and
    /// `attr()` (only `class` and `data-*` are not), or an element it cannot
    /// address.
    pub(super) fn invalidation(&self, batch: &Batch, theme_css: &str) -> Option<Changed> {
        let values = &self.state.values;
        let root = values.source_dom()?;
        if !batch.texts.is_empty()
            || *self.theme_css != *theme_css
            || self.traversal_limit().is_some()
        {
            return None;
        }
        let index = self.source_index();
        let mut changed = Changed::default();
        for (change, names) in batch.attribute_deltas() {
            if !open(&change.element, &root) {
                return None;
            }
            changed.elements.push(index.id(&change.element)?);
            for name in names {
                let (was, is) = (
                    attribute_value(&change.old, name),
                    attribute_value(&change.new, name),
                );
                let local = (*name.local).to_ascii_lowercase();
                let plain = name.ns == html5ever::ns!() && name.prefix.is_none();
                let data = local.starts_with("data-") && !local.starts_with("data-kamin");
                if !plain || !(local == "class" || data) {
                    return None;
                }
                if local == "class" {
                    changed
                        .classes
                        .extend(crate::dom::class_tokens::changed(was, is));
                }
                if !changed.attrs.contains(&local) {
                    changed.attrs.push(local);
                }
            }
        }
        Some(changed)
    }
}
