//! Source admission до parse, транзакционный update и scope старых кадров.

use super::*;
use crate::budget::{BudgetError, BudgetKind, DocumentLimits, ProcessLimits, ResourceGovernor};
use std::rc::Rc;

#[test]
fn source_boundary_includes_theme_and_failure_releases_document_slot() {
    let governor = ResourceGovernor::new(
        ProcessLimits::default()
            .lower(BudgetKind::Documents, 1)
            .unwrap(),
    );
    for bytes in [7, 8, 9] {
        let limits = DocumentLimits::default()
            .lower(BudgetKind::SourceBytes, 8)
            .unwrap();
        let html = "x".repeat(bytes - 1);
        let result = Document::builder(&html, " ")
            .limits(limits)
            .governor(&governor)
            .build();
        if bytes <= 8 {
            let doc = result.unwrap();
            assert_eq!(
                doc.revision_budget().used(BudgetKind::SourceBytes),
                bytes as u64
            );
            assert_eq!(doc.budget().limits().limit(BudgetKind::SourceBytes), 8);
        } else {
            let failure = result.err().unwrap();
            assert_eq!(failure.code(), "KAMINIDE_BUDGET_EXCEEDED");
            assert!(matches!(
                failure.reason,
                FailureReason::Budget(BudgetError::Exceeded(_))
            ));
        }
        assert_eq!(governor.used(BudgetKind::Documents), 0);
    }
    let limits = DocumentLimits::default()
        .lower(BudgetKind::SourceBytes, 1)
        .unwrap();
    let error = Document::builder("<broken", "")
        .path(std::path::Path::new("test.svg"))
        .limits(limits)
        .governor(&governor)
        .build()
        .err()
        .unwrap();
    assert_eq!(error.code(), "KAMINIDE_BUDGET_EXCEEDED");
    assert_eq!(governor.used(BudgetKind::Documents), 0);
    let failure = Document::builder("<broken", "")
        .path(std::path::Path::new("test.svg"))
        .governor(&governor)
        .build()
        .err()
        .unwrap();
    assert_eq!(failure.code(), "KAMINIDE_XML_SYNTAX");
    assert_eq!(governor.used(BudgetKind::Documents), 0);
}

#[test]
fn update_retains_document_ledger_but_work_belongs_to_each_attempt() {
    let governor = ResourceGovernor::default();
    let limits = DocumentLimits::default()
        .lower(BudgetKind::SourceBytes, 16)
        .unwrap();
    let mut doc = Document::builder("<p>a</p>", "")
        .limits(limits)
        .governor(&governor)
        .build()
        .unwrap();
    let old_state = doc.state.clone();
    let old_revision = doc.revision_budget();
    let live = doc.budget().reserve(BudgetKind::LiveBytes, 8).unwrap();
    let salt = doc.salt();
    let failure = doc.update(&"x".repeat(17), "").unwrap_err();
    assert_eq!(failure.report.retained_revision, Some(salt));
    assert_eq!(doc.salt(), salt);
    assert!(Rc::ptr_eq(&old_state, &doc.state));
    assert!(doc.update("<p>b</p>", "").unwrap().changed());
    assert_eq!(doc.revision_budget().used(BudgetKind::SourceBytes), 8);
    assert_eq!(old_revision.used(BudgetKind::SourceBytes), 8);
    assert_eq!(doc.budget().used(BudgetKind::LiveBytes), 8);
    assert_eq!(governor.used(BudgetKind::Documents), 1);
    drop(doc);
    drop(old_revision);
    drop(live);
    assert_eq!(governor.used(BudgetKind::Documents), 1);
    drop(old_state);
    assert_eq!(governor.used(BudgetKind::Documents), 0);
}
