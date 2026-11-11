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
        let handles = batch.elements.iter().map(|change| &change.element);
        let ids = super::adopt::source_ids(&values.traversal.elements(), handles);
        let mut changed = Changed::default();
        for (change, id) in batch.elements.iter().zip(ids) {
            if !open(&change.element, &root) {
                return None;
            }
            changed.elements.push(id?);
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
                    let tokens = |v: Option<&str>| -> Vec<String> {
                        let v = v.unwrap_or("").split_ascii_whitespace();
                        v.map(str::to_ascii_lowercase).collect()
                    };
                    let (a, b) = (tokens(was), tokens(is));
                    let moved = a
                        .iter()
                        .filter(|t| !b.contains(t))
                        .chain(b.iter().filter(|t| !a.contains(t)));
                    changed.classes.extend(moved.cloned());
                }
                if !changed.attrs.contains(&local) {
                    changed.attrs.push(local);
                }
            }
        }
        Some(changed)
    }
}

/// The value of the attribute `name` in `list`.
fn value<'a>(list: &'a [html5ever::Attribute], name: &html5ever::QualName) -> Option<&'a str> {
    list.iter().find(|a| a.name == *name).map(|a| &*a.value)
}
