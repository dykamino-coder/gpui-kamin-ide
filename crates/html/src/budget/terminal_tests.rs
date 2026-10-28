//! Первый типизированный отказ принадлежит ledger ревизии, а не DocState.

use super::*;
use crate::dom::outcome::{DocumentFailure, FailureReason};

#[test]
fn terminal_failure_is_shared_first_wins_and_new_revision_is_clean() {
    let document = DocumentBudget::new(Default::default(), Default::default()).unwrap();
    let revision = document.revision();
    let view = revision.clone();
    let failure = DocumentFailure::new(FailureReason::Allocation);
    revision.reject(failure);
    view.reject(DocumentFailure::new(FailureReason::CapacityOverflow));
    assert_eq!(view.failure(), Some(failure));
    assert!(document.revision().failure().is_none());
}

#[test]
fn graph_stack_live_refusal_survives_local_fallback_and_releases_storage() {
    let limits = DocumentLimits::default()
        .lower(BudgetKind::LiveBytes, 0)
        .unwrap();
    let budget = DocumentBudget::new(limits, Default::default()).unwrap();
    let revision = budget.revision();
    let work = crate::style::css::variable_values::Work::new(&revision);
    let mut stack = admission::Buffer::default();
    assert!(work.admit(stack.push(1u8, work.admission())).is_err());
    assert!(
        matches!(revision.failure().unwrap().reason, FailureReason::Budget(BudgetError::Exceeded(e)) if e.kind == BudgetKind::LiveBytes)
    );
    drop(stack);
    assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
}
