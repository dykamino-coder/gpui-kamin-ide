//! Attribute copies are admitted before their backing vectors are allocated.

use super::{Admission, Attribute, Tag, TreeBuilder, TreeSink};

impl<H: Clone, S: TreeSink<Handle = H>, A: Admission<H>> TreeBuilder<H, S, A> {
    pub(super) fn push_into<T>(
        &self,
        values: &std::cell::RefCell<Vec<T>>,
        value: T,
    ) -> Result<(), A::Error> {
        let mut values = values.borrow_mut();
        self.admission.reserve(&mut values, 1)?;
        values.push(value);
        Ok(())
    }

    pub(super) fn insert_into<T>(
        &self,
        values: &std::cell::RefCell<Vec<T>>,
        index: usize,
        value: T,
    ) -> Result<(), A::Error> {
        let mut values = values.borrow_mut();
        self.admission.reserve(&mut values, 1)?;
        values.insert(index, value);
        Ok(())
    }

    pub(super) fn clone_attrs(&self, attrs: &[Attribute]) -> Result<Vec<Attribute>, A::Error> {
        self.result()?;
        let mut copy = Vec::new();
        self.admission.reserve(&mut copy, attrs.len())?;
        copy.extend_from_slice(attrs);
        Ok(copy)
    }

    pub(super) fn clone_tag(&self, tag: &Tag) -> Result<Tag, A::Error> {
        Ok(Tag {
            kind: tag.kind,
            name: tag.name.clone(),
            self_closing: tag.self_closing,
            attrs: self.clone_attrs(&tag.attrs)?,
            had_duplicate_attributes: tag.had_duplicate_attributes,
        })
    }

    pub(super) fn equivalent_tags(&self, left: &Tag, right: &Tag) -> Result<bool, A::Error> {
        if left.kind != right.kind
            || left.name != right.name
            || left.attrs.len() != right.attrs.len()
        {
            return Ok(false);
        }
        let mut left = self.clone_attrs(&left.attrs)?;
        let mut right = self.clone_attrs(&right.attrs)?;
        left.sort_unstable();
        right.sort_unstable();
        Ok(left == right)
    }
}
