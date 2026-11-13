//! Индекс необходимых токенов предмета :has(); самый узкий список сохраняет preorder.

use super::*;
use crate::budget::admission::{Admission, Buffer};
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
    ) -> Result<Buffer<usize>, Error> {
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
        let mut out = Buffer::default();
        if let Some(list) = narrowest {
            admission.work(list.len())?;
            for &i in list {
                out.push(i, admission)?;
            }
        } else {
            admission.work(count)?;
            for i in 0..count {
                out.push(i, admission)?;
            }
        }
        Ok(out)
    }
}
