//! Capture ownership, payload accounting and deterministic bounded eviction.

use super::{BYTE_BUDGET, CAPACITY, CaptureCache};
use std::rc::Rc;

#[test]
fn hits_share_non_clone_captures_without_running_the_query() {
    struct Capture;
    let cache = CaptureCache::default();
    let first = cache.get_or_insert_with((0, 1), || vec![Capture], |_| 0);
    let hit = cache.get_or_insert_with((0, 1), || panic!("cache miss"), |_| 0);
    assert!(Rc::ptr_eq(&first, &hit));
}

#[test]
fn empty_results_are_cached_and_evict_only_the_oldest_entry() {
    let cache = CaptureCache::<u8>::default();
    let first = cache.get_or_insert_with((0, 1), Vec::new, |_| 0);
    let second = cache.get_or_insert_with((1, 2), Vec::new, |_| 0);
    for start in 2..=CAPACITY {
        cache.get_or_insert_with((start, start + 1), Vec::new, |_| 0);
    }
    assert_eq!(cache.len(), CAPACITY);
    let hit = cache.get_or_insert_with((1, 2), || panic!("evicted too much"), |_| 0);
    assert!(Rc::ptr_eq(&second, &hit));
    let replaced = cache.get_or_insert_with((0, 1), Vec::new, |_| 0);
    assert!(!Rc::ptr_eq(&first, &replaced));
    assert!(cache.state.borrow().bytes <= BYTE_BUDGET);
}

#[test]
fn heap_payloads_evict_by_bytes_before_the_entry_limit() {
    let cache = CaptureCache::default();
    let payload = || vec![vec![0u8; BYTE_BUDGET / 3]];
    let first = cache.get_or_insert_with((0, 1), payload, Vec::capacity);
    let second = cache.get_or_insert_with((1, 2), payload, Vec::capacity);
    cache.get_or_insert_with((2, 3), payload, Vec::capacity);
    assert_eq!(cache.len(), 2);
    assert!(cache.state.borrow().bytes <= BYTE_BUDGET);
    let hit = cache.get_or_insert_with((1, 2), || panic!("evicted too much"), Vec::capacity);
    assert!(Rc::ptr_eq(&second, &hit));
    assert_eq!(first[0].len(), BYTE_BUDGET / 3);
    assert!(!cache.state.borrow().entries.contains_key(&(0, 1)));
}

#[test]
fn oversize_capture_arrays_bypass_without_evicting_other_queries() {
    let cache = CaptureCache::<u8>::default();
    let small = cache.get_or_insert_with((0, 1), || vec![7], |_| 0);
    let large = cache.get_or_insert_with((1, 2), || vec![9; BYTE_BUDGET], |_| 0);
    assert_eq!(large.len(), BYTE_BUDGET);
    assert_eq!(cache.len(), 1);
    let hit = cache.get_or_insert_with((0, 1), || panic!("evicted by oversize"), |_| 0);
    assert!(Rc::ptr_eq(&small, &hit));
    let retried = cache.get_or_insert_with((1, 2), Vec::new, |_| 0);
    assert!(retried.is_empty());
}

#[test]
fn overflowing_heap_weights_bypass_the_cache() {
    let cache = CaptureCache::<u8>::default();
    assert_eq!(
        &*cache.get_or_insert_with((0, 1), || vec![1, 2], |_| usize::MAX),
        &[1, 2]
    );
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.state.borrow().bytes, 0);
}

#[test]
fn clearing_releases_the_budget_and_preserves_outstanding_results() {
    let cache = CaptureCache::default();
    let first = cache.get_or_insert_with((0, 1), || vec![4u8], |_| 0);
    cache.clear();
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.state.borrow().bytes, 0);
    assert!(cache.state.borrow().order.is_empty());
    let next = cache.get_or_insert_with((0, 1), || vec![5], |_| 0);
    assert_eq!(&*first, &[4]);
    assert_eq!(&*next, &[5]);
}
