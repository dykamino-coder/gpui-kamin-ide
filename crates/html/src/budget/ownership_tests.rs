//! Eviction, physical sharing и отказ allocation callback.

use super::*;
use std::sync::Arc;

fn document(governor: &ResourceGovernor) -> DocumentBudget {
    DocumentBudget::new(DocumentLimits::default(), governor.clone()).unwrap()
}

#[test]
fn cache_eviction_keeps_shared_physical_storage_until_last_owner() {
    let governor = ResourceGovernor::default();
    let a = document(&governor);
    let b = document(&governor);
    let storage = governor
        .allocate(BudgetKind::LiveBytes, 8, || Ok::<_, ()>(42))
        .unwrap();
    let cache = crate::cache::BoundedCache::<u8, Arc<SharedStorage<i32>>>::new(80, |_| 8);
    cache.insert(1, storage.clone());
    let first = a.admit(storage.clone()).unwrap();
    let second = b.admit(cache.get(&1).unwrap()).unwrap();
    let bypass = crate::cache::BoundedCache::<u8, Arc<SharedStorage<i32>>>::new(1, |_| 8);
    bypass.insert(1, storage.clone());
    assert!(bypass.get(&1).is_none());
    assert_eq!(governor.used(BudgetKind::LiveBytes), 8);
    cache.insert(2, storage.clone());
    assert!(cache.get(&1).is_none());
    assert_eq!(governor.used(BudgetKind::LiveBytes), 8);
    assert_eq!(a.used(BudgetKind::LiveBytes), 8);
    assert_eq!(b.used(BudgetKind::LiveBytes), 8);
    drop(storage);
    drop(cache);
    drop(first);
    assert_eq!(a.used(BudgetKind::LiveBytes), 0);
    assert_eq!(governor.used(BudgetKind::LiveBytes), 8);
    assert_eq!(*second.value(), 42);
    drop(second);
    assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
}

#[test]
fn payload_drops_before_its_physical_reservation() {
    struct Payload(ResourceGovernor);
    impl Drop for Payload {
        fn drop(&mut self) {
            assert_eq!(self.0.used(BudgetKind::LiveBytes), 8);
        }
    }
    let governor = ResourceGovernor::default();
    let storage = governor
        .allocate(BudgetKind::LiveBytes, 8, || {
            Ok::<_, ()>(Payload(governor.clone()))
        })
        .unwrap();
    drop(storage);
    assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
}

#[test]
fn shared_admission_failure_and_foreign_governor_release_correctly() {
    let governor = ResourceGovernor::default();
    let d = DocumentBudget::new(
        DocumentLimits::default()
            .lower(BudgetKind::LiveBytes, 7)
            .unwrap(),
        governor.clone(),
    )
    .unwrap();
    let storage = governor
        .allocate(BudgetKind::LiveBytes, 8, || Ok::<_, ()>(()))
        .unwrap();
    assert!(d.admit(storage.clone()).is_err());
    assert_eq!(d.used(BudgetKind::LiveBytes), 0);
    assert_eq!(governor.used(BudgetKind::LiveBytes), 8);
    let foreign = document(&ResourceGovernor::default());
    assert!(matches!(
        foreign.admit(storage.clone()),
        Err(BudgetError::ForeignGovernor)
    ));
    drop(storage);
    assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
}

#[test]
fn failed_allocation_and_unwind_release_and_admission_precedes_callback() {
    let governor = ResourceGovernor::new(
        ProcessLimits::default()
            .lower(BudgetKind::LiveBytes, 8)
            .unwrap(),
    );
    assert!(matches!(
        governor.allocate::<(), _>(BudgetKind::LiveBytes, 8, || Err("allocation")),
        Err(StorageError::Allocation("allocation"))
    ));
    assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
    assert!(matches!(
        governor.allocate(BudgetKind::LiveBytes, 9, || -> Result<(), ()> {
            panic!("must not allocate")
        }),
        Err(StorageError::Budget(_))
    ));
    assert!(
        std::panic::catch_unwind(|| {
            let _ = governor.allocate(BudgetKind::LiveBytes, 8, || -> Result<(), ()> {
                panic!("allocator panic")
            });
        })
        .is_err()
    );
    assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
}
