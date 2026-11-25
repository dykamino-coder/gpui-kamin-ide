//! The records one box keeps for band hosts, by key.
//!
//! A box keeps the record of its own host and of the synthetic hosts it
//! answers for (`content::band::owner`: the runs of inline content around
//! floats in one block). They differ by key (the walk to their boxes, with the
//! text they hold), so each box holds a short chain of records and
//! a host reads and writes only its own: with one record the hosts of a
//! block would replace each other's every frame and none would be read.

use super::Retained;
use crate::layout::reuse::BandKey;

/// Records one box keeps: its own host's and those of the runs of one
/// block (a list of items each wrapped around an icon float).
const HOSTS: usize = 16;

/// The record of `key` in the chain at `head`, or else the head, whose
/// differing input the `why` journal names.
pub(super) fn find<'a>(head: &'a Option<Box<Retained>>, key: &BandKey) -> Option<&'a Retained> {
    let first = head.as_deref()?;
    let mut at = Some(first);
    while let Some(record) = at {
        if record.key.same(key) {
            return Some(record);
        }
        at = record.other.as_deref();
    }
    Some(first)
}

/// The record of `key` in the chain at `head` for a write at publication
/// `clock`: kept while its subtree (box epoch `epoch`) did not change,
/// otherwise made anew by `fresh` in its place; a new key goes first and the
/// chain keeps the `HOSTS` latest.
pub(super) fn entry<'a>(
    head: &'a mut Option<Box<Retained>>,
    key: &BandKey,
    epoch: u32,
    clock: u32,
    fresh: impl FnOnce() -> Retained,
) -> &'a mut Retained {
    let position = {
        let mut at = head.as_deref();
        let mut index = 0;
        loop {
            match at {
                Some(record) if record.key.same(key) => break Some(index),
                Some(record) => at = record.other.as_deref(),
                None => break None,
            }
            index += 1;
        }
    };
    let Some(index) = position else {
        let mut record = Box::new(fresh());
        record.other = head.take();
        truncate(&mut record, HOSTS);
        return head.insert(record);
    };
    let mut slot = head;
    for _ in 0..index {
        slot = &mut slot.as_mut().expect("the record was found here").other;
    }
    let record = slot.as_mut().expect("the record was found here");
    if epoch > record.at || record.at > clock {
        let other = record.other.take();
        **record = fresh();
        record.other = other;
    }
    record
}

/// Keep at most `len` records in the chain from `record`.
fn truncate(record: &mut Retained, len: usize) {
    let mut at = record;
    for _ in 1..len {
        match at.other.as_deref_mut() {
            Some(next) => at = next,
            None => return,
        }
    }
    at.other = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn record(width: f32, at: u32) -> Retained {
        Retained::for_tests(Rc::new(BandKey::for_tests(width)), at)
    }

    fn len(head: &Option<Box<Retained>>) -> usize {
        let mut at = head.as_deref();
        let mut n = 0;
        while let Some(r) = at {
            n += 1;
            at = r.other.as_deref();
        }
        n
    }

    #[test]
    fn hosts_of_one_box_keep_their_own_records() {
        let mut head = None;
        for width in [1.0, 2.0, 1.0, 2.0] {
            let key = BandKey::for_tests(width);
            let kept = entry(&mut head, &key, 1, 1, || record(width, 1));
            assert!(kept.key.same(&key));
        }
        assert_eq!(len(&head), 2, "a key written again reuses its record");
        assert!(find(&head, &BandKey::for_tests(1.0)).unwrap().key.same(&BandKey::for_tests(1.0)));
        for width in 3..10 {
            let key = BandKey::for_tests(width as f32);
            entry(&mut head, &key, 1, 1, || record(width as f32, 1));
        }
        assert_eq!(len(&head), HOSTS);
    }

    #[test]
    fn a_changed_subtree_renews_only_its_record() {
        let mut head = None;
        let (one, two) = (BandKey::for_tests(1.0), BandKey::for_tests(2.0));
        entry(&mut head, &one, 1, 1, || record(1.0, 1));
        entry(&mut head, &two, 1, 1, || record(2.0, 1));
        let renewed = entry(&mut head, &one, 5, 5, || record(1.0, 5));
        assert_eq!(renewed.at, 5);
        assert_eq!(find(&head, &two).unwrap().at, 1);
        assert_eq!(len(&head), 2);
    }
}
