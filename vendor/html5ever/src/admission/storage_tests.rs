//! Boundary, transfer and rollback checks for parser backing allocations.

use super::{
    deque::StorageDeque,
    storage::{Lease, ReserveError, Storage},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};

struct Ticket {
    bytes: usize,
    live: Arc<AtomicUsize>,
}
impl Drop for Ticket {
    fn drop(&mut self) {
        self.live.fetch_sub(self.bytes, SeqCst);
    }
}
fn lease(bytes: usize, live: &Arc<AtomicUsize>) -> Lease {
    live.fetch_add(bytes, SeqCst);
    Some(Arc::new(Ticket {
        bytes,
        live: live.clone(),
    }))
}

#[test]
fn vector_capacity_boundaries_reject_before_mutation_and_keep_previous_owner() {
    for length in [3, 4, 5] {
        let live = Arc::new(AtomicUsize::new(0));
        let mut values = Storage::<u64>::default();
        values
            .try_reserve_with(4, |bytes| Ok::<_, ()>(lease(bytes, &live)))
            .unwrap();
        values.extend(0..3);
        let before = live.load(SeqCst);
        let result = values.try_reserve_with(length - 3, |_| Err("limit"));
        if length <= 4 {
            result.unwrap();
        } else {
            assert!(matches!(result, Err(ReserveError::Admission("limit"))));
        }
        assert_eq!(&**values, &[0, 1, 2]);
        assert_eq!(live.load(SeqCst), before);
        drop(values);
        assert_eq!(live.load(SeqCst), 0);
    }
}

#[test]
fn growth_charges_both_generations_and_iterator_keeps_its_storage() {
    let live = Arc::new(AtomicUsize::new(0));
    let mut values = Storage::<u64>::default();
    values
        .try_reserve_with(4, |bytes| Ok::<_, ()>(lease(bytes, &live)))
        .unwrap();
    values.extend(0..4);
    let old = live.load(SeqCst);
    values
        .try_reserve_with(1, |bytes| {
            assert_eq!(live.load(SeqCst), old);
            let ticket = lease(bytes, &live);
            assert_eq!(live.load(SeqCst), old + bytes);
            Ok::<_, ()>(ticket)
        })
        .unwrap();
    assert_eq!(live.load(SeqCst), size_of::<[u64; 8]>());
    let mut iter = values.into_iter();
    assert_eq!(iter.next(), Some(0));
    assert_ne!(live.load(SeqCst), 0);
    drop(iter);
    assert_eq!(live.load(SeqCst), 0);
}

#[test]
fn deque_growth_and_rejection_preserve_order_and_release_storage() {
    let live = Arc::new(AtomicUsize::new(0));
    let mut values = StorageDeque::<u64>::new();
    for count in [3, 4, 5] {
        values
            .try_reserve_with(1, |bytes| Ok::<_, ()>(lease(bytes, &live)))
            .unwrap();
        values.push_back(count);
    }
    assert_eq!(values.pop_front(), Some(3));
    values
        .try_reserve_with(3, |bytes| Ok::<_, ()>(lease(bytes, &live)))
        .unwrap();
    let before = live.load(SeqCst);
    assert!(matches!(
        values.try_reserve_with(usize::MAX, |_| Ok::<_, ()>(None)),
        Err(ReserveError::Overflow)
    ));
    assert_eq!(values.iter().copied().collect::<Vec<_>>(), [4, 5]);
    assert_eq!(live.load(SeqCst), before);
    drop(values);
    assert_eq!(live.load(SeqCst), 0);
}
