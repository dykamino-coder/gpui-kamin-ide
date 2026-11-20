//! Children of one removed `<html>`/`<body>` wrapper whose cascades share their
//! groups take one merged style (`inherit_unwrapped`): the identity the cascade
//! gave them (style sharing) reaches the computed stage, whose sharing cache
//! (`materialize::sharing`) then computes each distinct style once.

use crate::style::computed::{Computed, Storage};
use crate::values::ValueCx;

/// Distinct recent cascades kept, newest last; repeated children of a page are
/// interleaved runs of a few styles, as in the computed stage's cache.
const CAPACITY: usize = 8;

/// Merged styles of the children of one wrapper, while its children are taken.
#[derive(Default)]
pub struct Unwrapped {
    recent: Vec<(Storage, Computed)>,
    next: usize,
}

impl Unwrapped {
    /// `inherit_unwrapped(wrapper, own)`, shared with an earlier child of the
    /// same wrapper whose cascade has the same groups.
    pub fn inherit(
        &mut self,
        wrapper: &crate::style::computed::BoxStyle,
        own: &Computed,
        vcx: ValueCx<'_>,
    ) -> Computed {
        // The element's own decoration lines carry its box's identity into
        // the groups (`decorate`): such a merge belongs to its element only.
        let identity_free = own.reset_text_box.td_lines.is_none_or(|l| l == 0);
        if identity_free
            && let Some((_, merged)) = self.recent.iter().rev().find(|(k, _)| k.matches(own))
        {
            let mut merged = merged.clone();
            merged.decor_box = own.decor_box;
            return merged;
        }
        let merged = super::inherit_unwrapped(wrapper, own, vcx);
        if identity_free {
            let entry = (Storage::of(own), merged.clone());
            if self.recent.len() < CAPACITY {
                self.recent.push(entry);
            } else {
                self.recent[self.next] = entry;
                self.next = (self.next + 1) % CAPACITY;
            }
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    //! The merged style of repeated children: shared groups, own identity, equal values.
    use super::*;
    use crate::style::computed::BoxStyle;
    use crate::style::css::Media;
    use crate::values::DocumentValues;
    use std::rc::Rc;

    #[test]
    fn children_with_shared_cascades_share_the_merged_groups() {
        let doc = DocumentValues::detached(Media::default());
        let vcx = doc.values();
        let mut wrapper = doc.initial_style();
        wrapper.apply_one("font-size", "20px", vcx);
        let wrapper = BoxStyle::from(wrapper);
        let mut own = doc.initial_style();
        own.apply_one("width", "2em", vcx);
        own.decor_box = 1;
        let mut sibling = own.clone();
        sibling.decor_box = 2;
        let mut memo = Unwrapped::default();
        let first = memo.inherit(&wrapper, &own, vcx);
        let second = memo.inherit(&wrapper, &sibling, vcx);
        assert!(first.shares_storage(&second));
        assert_eq!(second.decor_box, 2);
        let fresh = super::super::inherit_unwrapped(&wrapper, &sibling, vcx);
        assert_eq!(second, fresh);
        assert!(Rc::ptr_eq(&first.inh.0, &second.inh.0));
    }

    #[test]
    fn own_decoration_lines_are_merged_per_element() {
        let doc = DocumentValues::detached(Media::default());
        let vcx = doc.values();
        let wrapper = BoxStyle::from(doc.initial_style());
        let mut own = doc.initial_style();
        own.apply_one("text-decoration-line", "underline", vcx);
        own.decor_box = 1;
        let mut sibling = own.clone();
        sibling.decor_box = 2;
        let mut memo = Unwrapped::default();
        memo.inherit(&wrapper, &own, vcx);
        let second = memo.inherit(&wrapper, &sibling, vcx);
        let fresh = super::super::inherit_unwrapped(&wrapper, &sibling, vcx);
        assert_eq!(second, fresh);
    }
}
