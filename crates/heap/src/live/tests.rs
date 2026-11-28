use super::*;

#[test]
fn reuse_and_realloc_have_distinct_lifetimes_but_failure_preserves_identity() {
    let mut ledger = Ledger::new(4, 20);
    let first = ledger.allocate(16, 10, Stack::default()).unwrap();
    ledger.reallocate(16, 0, 30, Stack::default());
    assert_eq!(ledger.blocks().next().unwrap().id, first);
    assert_eq!(ledger.totals().live_bytes, 10);
    ledger.reallocate(16, 16, 30, Stack::default());
    let second = ledger.blocks().next().unwrap().id;
    assert_ne!(first, second);
    ledger.free(16);
    let third = ledger.allocate(16, 5, Stack::default()).unwrap();
    assert_ne!(second, third);
    let peak = ledger.peak().unwrap();
    assert_eq!(peak.len(), 1);
    assert_eq!((peak[0].id, peak[0].bytes), (second, 30));
    assert_eq!(ledger.totals().live_bytes, 5);
}

#[test]
fn bounds_and_lost_history_are_explicit_and_capacity_can_be_reused() {
    let mut ledger = Ledger::new(1, 2);
    ledger.allocate(16, 10, Stack::default());
    assert!(ledger.allocate(32, 20, Stack::default()).is_none());
    assert_eq!(ledger.totals().missing_allocations, 1);
    ledger.free(16);
    assert!(ledger.allocate(32, 20, Stack::default()).is_some());
    assert_eq!(ledger.totals().history_dropped, 1);
    assert!(ledger.peak().is_none());
    assert_eq!(ledger.totals().live_bytes, 20);
}
