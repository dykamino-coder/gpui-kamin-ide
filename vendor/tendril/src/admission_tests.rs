//! Physical leases survive slices and release only after the last owner.

use super::*;
use std::any::Any;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Lease {
    used: Arc<AtomicUsize>,
    bytes: usize,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}

fn admission(
    used: &Arc<AtomicUsize>,
    limit: usize,
) -> impl FnOnce(usize) -> Result<Arc<dyn Any + Send + Sync>, usize> + '_ {
    move |bytes| {
        used.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
            old.checked_add(bytes).filter(|sum| *sum <= limit)
        })
        .map_err(|old| old + bytes)?;
        Ok(Arc::new(Lease {
            used: used.clone(),
            bytes,
        }))
    }
}

#[test]
fn exact_allocation_boundary_and_unchanged_failure() {
    let used = Arc::new(AtomicUsize::new(0));
    let mut sample = StrTendril::new();
    sample
        .try_reserve_with(9, admission(&used, usize::MAX))
        .unwrap();
    let bytes = sample.allocation_size();
    assert_eq!(used.load(Ordering::Relaxed), bytes);
    drop(sample);
    assert_eq!(used.load(Ordering::Relaxed), 0);
    for limit in [bytes - 1, bytes, bytes + 1] {
        let mut text = StrTendril::from("KEEP");
        let result = text.try_reserve_with(5, admission(&used, limit));
        assert_eq!(result.is_ok(), limit >= bytes);
        assert_eq!(&*text, "KEEP");
        assert_eq!(
            used.load(Ordering::Relaxed),
            if result.is_ok() { bytes } else { 0 }
        );
        drop(text);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn shared_slice_keeps_parent_charge_and_cow_overlap() {
    let used = Arc::new(AtomicUsize::new(0));
    let mut parent = StrTendril::new();
    parent
        .try_reserve_with(100, admission(&used, usize::MAX))
        .unwrap();
    parent.push_slice("0123456789KEEP");
    let bytes = parent.allocation_size();
    let mut child = parent.subtendril(0, 9);
    assert_eq!(child.allocation_size(), bytes);
    drop(parent);
    assert_eq!(used.load(Ordering::Relaxed), bytes);
    let shared = child.clone();
    let result = child.try_reserve_with(1, admission(&used, bytes));
    assert!(matches!(result, Err(ReserveError::Admission(_))));
    assert!(child.is_shared_with(&shared));
    assert_eq!(&*child, "012345678");
    child
        .try_reserve_with(1, admission(&used, usize::MAX))
        .unwrap();
    assert!(!child.is_shared_with(&shared));
    assert_eq!(
        used.load(Ordering::Relaxed),
        bytes + child.allocation_size()
    );
    child.push_slice("X");
    assert_eq!(&*shared, "012345678");
    drop(shared);
    assert_eq!(used.load(Ordering::Relaxed), child.allocation_size());
    drop(child);
    assert_eq!(used.load(Ordering::Relaxed), 0);
}

#[test]
fn independent_ledgers_and_inline_storage() {
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    let mut a = StrTendril::new();
    let mut b = StrTendril::new();
    a.try_reserve_with(8, |_| -> Result<Arc<dyn Any + Send + Sync>, ()> {
        panic!("inline data must not allocate")
    })
    .unwrap();
    a.try_reserve_with(9, admission(&first, usize::MAX))
        .unwrap();
    b.try_reserve_with(17, admission(&second, usize::MAX))
        .unwrap();
    drop(a);
    assert_eq!(first.load(Ordering::Relaxed), 0);
    assert_eq!(second.load(Ordering::Relaxed), b.allocation_size());
    drop(b);
    assert_eq!(second.load(Ordering::Relaxed), 0);
}
