//! The block records over a rebuild of the same source DOM (`layout::reuse::Store::rebase`).

use super::mutation::{Batch, open};
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
        next.root_phases = self.root_phases.clone();
    }

    /// The names the attribute mutations of `batch` change, for the rebuild
    /// of this document's source DOM under the same theme to find the
    /// elements they restyle (`dom::invalidation`); `None`: the batch changes
    /// texts or attributes that the engine reads outside selectors and
    /// `attr()` (only `class` and `data-*` are not); an element without a box
    /// is numbered `UNADDRESSED`.
    pub(super) fn invalidation(&self, batch: &Batch, theme_css: &str) -> Option<Changed> {
        let values = &self.state.values;
        let root = values.source_dom()?;
        if !batch.texts.is_empty()
            || *self.theme_css != *theme_css
            || self.traversal_limit().is_some()
        {
            return None;
        }
        let index = self.source_index()?;
        let mut changed = Changed::default();
        for change in &batch.elements {
            if !open(&change.element, &root) {
                return None;
            }
            // An element the build did not enter (under `display: none`, a
            // dropped tag) has no box (css-display-3 §2.5): it is still a
            // mutated element of the restyle, numbered `UNADDRESSED`, whose
            // probe decides whether it gains one.
            let id = index.id(&change.element);
            changed.elements.push(id.unwrap_or(UNADDRESSED));
            let mut seen = Vec::new();
            for attr in change.old.iter().chain(&change.new) {
                let (was, is) = (
                    value(&change.old, &attr.name),
                    value(&change.new, &attr.name),
                );
                let local = (*attr.name.local).to_ascii_lowercase();
                if was == is || seen.contains(&local) {
                    continue;
                }
                seen.push(local.clone());
                let plain = attr.name.ns == html5ever::ns!() && attr.name.prefix.is_none();
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

/// The number of a mutated element without a box: box numbers start at one
/// (`SourceIndex`), and the box index keeps no box of number zero (`Boxes`).
const UNADDRESSED: u64 = 0;

/// The value of the attribute `name` in `list`.
fn value<'a>(list: &'a [html5ever::Attribute], name: &html5ever::QualName) -> Option<&'a str> {
    list.iter().find(|a| a.name == *name).map(|a| &*a.value)
}
