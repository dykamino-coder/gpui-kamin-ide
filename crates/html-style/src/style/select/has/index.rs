//! Индекс необходимых токенов предмета :has(); самый узкий список сохраняет preorder.

use super::postings::Candidates;
use super::*;
use crate::budget::admission::Admission;
use crate::style::select::ascii_lower as lower;
use std::collections::HashMap;

#[derive(Default)]
pub(in crate::style::select) struct Index {
    buckets: [HashMap<String, Vec<usize>>; 4],
    leases: Vec<Option<crate::budget::LiveReservation>>,
}

impl Index {
    pub fn insert_handle(
        &mut self,
        handle: &Handle,
        i: usize,
        admission: &Admission,
    ) -> Result<(), Error> {
        let markup5ever_rcdom::NodeData::Element { name, attrs, .. } = &handle.data else {
            return Ok(());
        };
        let local = &*name.local;
        let local = local
            .split_once(':')
            .filter(|(_, suffix)| !suffix.is_empty())
            .map_or(local, |(_, suffix)| suffix);
        self.add(2, &lower(local), i, admission)?;
        for attr in attrs.borrow().iter() {
            let key = &*attr.name.local;
            self.add(3, &lower(key), i, admission)?;
            if key == "id" {
                self.add(0, &attr.value, i, admission)?;
            } else if key == "class" {
                for class in attr.value.split_whitespace() {
                    self.add(1, class, i, admission)?;
                }
            }
        }
        Ok(())
    }

    fn add(
        &mut self,
        kind: usize,
        key: &str,
        i: usize,
        admission: &Admission,
    ) -> Result<(), Error> {
        admission.work(1)?;
        let lease =
            admission.storage(key.len().checked_add(384).ok_or(Error::CapacityOverflow)?)?;
        self.leases.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.buckets[kind]
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        // An existing token is found without copying its text.
        let bucket = match self.buckets[kind].get_mut(key) {
            Some(bucket) => bucket,
            None => self.buckets[kind].entry(key.to_string()).or_default(),
        };
        if bucket.last() == Some(&i) {
            return Ok(());
        }
        bucket.try_reserve(1).map_err(|_| Error::Allocation)?;
        bucket.push(i);
        self.leases.push(lease);
        Ok(())
    }

    pub fn candidates(
        &self,
        sel: &Selector,
        count: usize,
        admission: &Admission,
    ) -> Result<Candidates<'_>, Error> {
        let mut narrowest: Option<&[usize]> = None;
        let empty: &[usize] = &[];
        let mut consider = |kind: usize, key: &str| {
            let list = self.buckets[kind].get(key).map_or(empty, Vec::as_slice);
            if narrowest.is_none_or(|old| list.len() < old.len()) {
                narrowest = Some(list);
            }
        };
        if let Some(id) = &sel.id {
            consider(0, id);
        }
        for class in &sel.classes {
            consider(1, class);
        }
        if let Some(tag) = &sel.tag {
            consider(2, &lower(tag));
        }
        for attr in &sel.attrs {
            consider(3, &lower(&attr.name));
        }
        let out = Candidates::new(narrowest, count);
        admission.work(out.len())?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{BudgetKind, RevisionBudget};

    #[test]
    fn repeated_posting_queries_borrow_the_existing_storage() {
        let revision = RevisionBudget::standalone();
        let admission = Admission::new(revision.clone());
        let mut index = Index::default();
        for i in 0..1024 {
            index.add(1, "shared", i, &admission).unwrap();
        }
        let document = revision.document();
        let remaining =
            document.limits().limit(BudgetKind::LiveBytes) - document.used(BudgetKind::LiveBytes);
        let _occupied = document
            .reserve(BudgetKind::LiveBytes, remaining - 1)
            .unwrap();
        let sel = Selector::parse(".shared").unwrap();
        for _ in 0..8 {
            let candidates = index.candidates(&sel, 1024, &admission).unwrap();
            assert_eq!(candidates.len(), 1024);
        }
    }
}
