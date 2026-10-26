//! Границы каждого счётчика и инварианты scopes; маленькие host quotas.

use super::ledger::Ledger;
use super::*;

fn document(governor: &ResourceGovernor) -> DocumentBudget {
    DocumentBudget::new(DocumentLimits::default(), governor.clone()).unwrap()
}

#[test]
fn every_counter_boundary_and_overflow() {
    for kind in BudgetKind::ALL {
        let ledger = Ledger::new(BudgetScope::Revision, [8; 12]);
        match kind.class() {
            BudgetClass::Structural => {
                assert!(ledger.check(kind, 7).is_ok());
                assert!(ledger.check(kind, 8).is_ok());
                assert!(ledger.check(kind, 9).is_err());
                assert!(ledger.check(kind, u64::MAX).is_err());
                assert_eq!(ledger.used(kind), 8);
            }
            BudgetClass::Monotonic => {
                ledger.charge(kind, 7).unwrap();
                ledger.charge(kind, 1).unwrap();
                assert!(ledger.charge(kind, 1).is_err());
                assert_eq!(ledger.used(kind), 8);
            }
            BudgetClass::Live => {
                let a = ledger.reserve(kind, 7).unwrap();
                let b = ledger.reserve(kind, 1).unwrap();
                assert!(ledger.reserve(kind, 1).is_err());
                assert_eq!(ledger.used(kind), 8);
                drop(a);
                assert_eq!(ledger.used(kind), 1);
                drop(b);
                assert_eq!(ledger.used(kind), 0);
            }
        }
        if kind.class() != BudgetClass::Structural {
            let ledger = Ledger::new(BudgetScope::Document, [8; 12]);
            ledger.charge(kind, 1).unwrap();
            let Err(BudgetError::Exceeded(error)) = ledger.charge(kind, u64::MAX) else {
                panic!("expected overflow");
            };
            assert!(error.overflow);
            assert_eq!(
                (
                    error.kind,
                    error.unit,
                    error.scope,
                    error.limit,
                    error.consumed,
                    error.increment
                ),
                (kind, kind.unit(), BudgetScope::Document, 8, 1, u64::MAX)
            );
            assert_eq!(ledger.used(kind), 1);
        }
    }
}

#[test]
fn profiles_are_finite_and_only_lowerable() {
    let limits = DocumentLimits::default();
    let process = ProcessLimits::default();
    for kind in BudgetKind::ALL {
        assert!(limits.limit(kind) > 0 && limits.limit(kind) < u64::MAX);
        assert!(limits.lower(kind, limits.limit(kind) + 1).is_err());
        assert!(limits.lower(kind, u64::MAX).is_err());
        assert_eq!(limits.lower(kind, 0).unwrap().limit(kind), 0);
        if kind.class() == BudgetClass::Live {
            assert_eq!(process.limit(kind), 4 * limits.limit(kind));
            assert!(process.lower(kind, process.limit(kind) + 1).is_err());
        } else {
            assert!(process.lower(kind, 0).is_err());
        }
    }
    assert_eq!(
        (DocumentLimits::BOX_DEPTH, DocumentLimits::LAYOUT_DEPTH),
        (2052, 1026)
    );
}

#[test]
fn work_is_monotonic_within_revision_and_generations_share_live_quota() {
    let governor = ResourceGovernor::default();
    let d = document(&governor);
    let revision = d.revision();
    revision.consume(BudgetKind::SourceBytes, 7).unwrap();
    let job = revision.clone();
    job.consume(BudgetKind::SourceBytes, 3).unwrap();
    drop(job);
    assert_eq!(revision.used(BudgetKind::SourceBytes), 10);
    assert_eq!(d.revision().used(BudgetKind::SourceBytes), 0);
    let allocation = d.reserve(BudgetKind::LiveBytes, 8).unwrap();
    drop(d);
    assert_eq!(revision.document().used(BudgetKind::LiveBytes), 8);
    assert_eq!(governor.used(BudgetKind::Documents), 1);
    drop(revision);
    assert_eq!(governor.used(BudgetKind::Documents), 1);
    drop(allocation);
    assert_eq!(governor.used(BudgetKind::Documents), 0);
    assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
}

#[test]
fn process_failure_rolls_back_local_live_but_never_work() {
    let governor = ResourceGovernor::new(
        ProcessLimits::default()
            .lower(BudgetKind::LiveBytes, 8)
            .unwrap(),
    );
    let a = document(&governor);
    let b = document(&governor);
    let live = a.reserve(BudgetKind::LiveBytes, 8).unwrap();
    assert!(b.reserve(BudgetKind::LiveBytes, 1).is_err());
    assert_eq!(b.used(BudgetKind::LiveBytes), 0);
    assert_eq!(governor.used(BudgetKind::LiveBytes), 8);
    drop(live);
    assert!(b.reserve(BudgetKind::LiveBytes, 8).is_ok());
    let work = a.revision();
    work.consume(BudgetKind::ResourceRequests, 1).unwrap();
    assert!(
        work.consume(BudgetKind::ResourceRequests, u64::MAX)
            .is_err()
    );
    assert_eq!(work.used(BudgetKind::ResourceRequests), 1);
    assert!(work.consume(BudgetKind::LiveBytes, 1).is_err());
    assert!(work.check_maximum(BudgetKind::ResourceBytes, 1).is_err());
    assert!(a.reserve(BudgetKind::ResourceBytes, 1).is_err());
}

#[test]
fn concurrent_documents_have_independent_ledgers_and_one_host_quota() {
    let governor = ResourceGovernor::new(
        ProcessLimits::default()
            .lower(BudgetKind::LiveBytes, 16)
            .unwrap(),
    );
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let (governor, barrier) = (&governor, &barrier);
            scope.spawn(move || {
                let d = document(governor);
                let live = d.reserve(BudgetKind::LiveBytes, 8).unwrap();
                barrier.wait();
                assert_eq!(d.used(BudgetKind::LiveBytes), 8);
                assert_eq!(governor.used(BudgetKind::LiveBytes), 16);
                assert!(d.reserve(BudgetKind::LiveBytes, 1).is_err());
                barrier.wait();
                drop(live);
            });
        }
    });
    assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
    assert_eq!(governor.used(BudgetKind::Documents), 0);
}

#[test]
fn document_slots_release_after_rejection_and_last_view() {
    let governor = ResourceGovernor::new(
        ProcessLimits::default()
            .lower(BudgetKind::Documents, 2)
            .unwrap(),
    );
    let a = document(&governor);
    let b = document(&governor);
    assert!(DocumentBudget::new(DocumentLimits::default(), governor.clone()).is_err());
    let view = a.clone();
    drop(a);
    assert_eq!(governor.used(BudgetKind::Documents), 2);
    drop(view);
    drop(b);
    assert_eq!(governor.used(BudgetKind::Documents), 0);
    let deny = DocumentLimits::default()
        .lower(BudgetKind::Documents, 0)
        .unwrap();
    assert!(DocumentBudget::new(deny, governor.clone()).is_err());
    assert_eq!(governor.used(BudgetKind::Documents), 0);
}
