//! Attribute copies are admitted before their backing vectors are allocated.

use super::{Admission, Attribute, Storage, StrTendril, Tag, TreeBuilder, TreeSink};
use crate::admission::Operation;

impl<H: Clone, S: TreeSink<Handle = H>, A: Admission<S>> TreeBuilder<H, S, A> {
    pub(super) fn work(&self, count: usize) -> Result<(), A::Error> {
        self.admission.check(Operation::Work(count))
    }

    pub(super) fn find<I, P>(&self, iter: I, mut predicate: P) -> Result<Option<I::Item>, A::Error>
    where
        I: IntoIterator,
        P: FnMut(&I::Item) -> bool,
    {
        for item in iter {
            self.work(1)?;
            if predicate(&item) {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    pub(super) fn any_not_whitespace(&self, text: &StrTendril) -> Result<bool, A::Error> {
        for c in text.chars() {
            self.work(c.len_utf8())?;
            if !c.is_ascii_whitespace() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn admit_whitespace_run(&self, text: &StrTendril) -> Result<(), A::Error> {
        let mut class = None;
        for c in text.chars() {
            // This scan and the canonical pop_front_char_run inspect the same prefix.
            self.work(2 * c.len_utf8())?;
            let current = c.is_ascii_whitespace();
            if class.is_some_and(|first| first != current) {
                break;
            }
            class = Some(current);
        }
        Ok(())
    }

    pub(super) fn push_into<T>(
        &self,
        values: &std::cell::RefCell<Storage<T>>,
        value: T,
    ) -> Result<(), A::Error> {
        let mut values = values.borrow_mut();
        self.admission.reserve(&mut values, 1)?;
        values.push(value);
        Ok(())
    }

    pub(super) fn insert_into<T>(
        &self,
        values: &std::cell::RefCell<Storage<T>>,
        index: usize,
        value: T,
    ) -> Result<(), A::Error> {
        let mut values = values.borrow_mut();
        self.admission.reserve(&mut values, 1)?;
        self.work(values.len() - index)?;
        values.insert(index, value);
        Ok(())
    }

    pub(super) fn clone_attrs(&self, attrs: &[Attribute]) -> Result<Storage<Attribute>, A::Error> {
        self.result()?;
        let mut copy = Storage::default();
        self.admission.reserve(&mut copy, attrs.len())?;
        self.work(attrs.len())?;
        copy.extend_from_slice(attrs);
        Ok(copy)
    }

    pub(super) fn remove_from<T>(
        &self,
        values: &std::cell::RefCell<Storage<T>>,
        index: usize,
    ) -> Result<T, A::Error> {
        let mut values = values.borrow_mut();
        self.work(values.len() - index - 1)?;
        Ok(values.remove(index))
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

    fn compare_attrs(
        &self,
        left: &Attribute,
        right: &Attribute,
    ) -> Result<std::cmp::Ordering, A::Error> {
        // Atom ordering and values may compare UTF-8 bytes. Charge each
        // component separately so even an aggregate usize sum cannot overflow.
        for name in [&left.name, &right.name] {
            self.work(name.local.len())?;
            self.work(name.ns.len())?;
            if let Some(prefix) = &name.prefix {
                self.work(prefix.len())?;
            }
        }
        self.work(left.value.len())?;
        self.work(right.value.len())?;
        self.work(1)?;
        Ok(left.cmp(right))
    }

    // In-place heapsort allows a comparison to reject without panic/unwind.
    // Attribute order is immaterial here; only multiset equality is observed.
    fn sort_attrs(&self, attrs: &mut [Attribute]) -> Result<(), A::Error> {
        for start in (0..attrs.len() / 2).rev() {
            self.sift_attrs(attrs, start, attrs.len())?;
        }
        for end in (1..attrs.len()).rev() {
            self.work(1)?;
            attrs.swap(0, end);
            self.sift_attrs(attrs, 0, end)?;
        }
        Ok(())
    }

    fn sift_attrs(
        &self,
        attrs: &mut [Attribute],
        mut root: usize,
        end: usize,
    ) -> Result<(), A::Error> {
        while root < end / 2 {
            let mut child = 2 * root + 1;
            if child + 1 < end
                && self
                    .compare_attrs(&attrs[child], &attrs[child + 1])?
                    .is_lt()
            {
                child += 1;
            }
            if !self.compare_attrs(&attrs[root], &attrs[child])?.is_lt() {
                break;
            }
            attrs.swap(root, child);
            root = child;
        }
        Ok(())
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
        self.sort_attrs(&mut left)?;
        self.sort_attrs(&mut right)?;
        for (left, right) in left.iter().zip(right.iter()) {
            if !self.compare_attrs(left, right)?.is_eq() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
