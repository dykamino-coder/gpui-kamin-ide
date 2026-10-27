//! Aggregate ревизии в ранних проходах сборки: подстановки и ресурсы фона. Исчерпание
//! отклоняет кандидата (первая загрузка и update), граница допускает документ.

use super::*;
use crate::budget::{BudgetError, BudgetKind, BudgetScope, DocumentLimits, ResourceGovernor};

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYPj/HwADAgH/eL9GtQAAAABJRU5ErkJggg==";

fn build(html: &str, kind: BudgetKind, limit: u64) -> Result<Document, DocumentFailure> {
    let limits = DocumentLimits::default().lower(kind, limit).unwrap();
    Document::builder(html, "")
        .limits(limits)
        .governor(&ResourceGovernor::default())
        .build()
}

fn revision_exceeded(failure: &DocumentFailure, kind: BudgetKind) -> bool {
    matches!(
        failure.reason,
        FailureReason::Budget(BudgetError::Exceeded(e))
            if e.kind == kind && e.scope == BudgetScope::Revision
    )
}

/// Сколько единиц `kind` тратит сборка `html` в стандартном профиле.
fn spent(html: &str, kind: BudgetKind) -> u64 {
    let doc = build(html, kind, DocumentLimits::default().limit(kind)).unwrap();
    doc.revision_budget().used(kind)
}

#[test]
fn substitution_aggregate_boundary_decides_the_whole_revision() {
    let html = r#"<div style="--a: green; color: var(--a)"></div>"#;
    let need = spent(html, BudgetKind::SubstitutionBytes);
    assert!(need > 0);
    for (limit, ok) in [(need - 1, false), (need, true), (need + 1, true)] {
        match build(html, BudgetKind::SubstitutionBytes, limit) {
            Ok(doc) => assert!(ok && doc.revision_budget().exhausted().is_none()),
            Err(failure) => {
                assert!(!ok);
                assert!(revision_exceeded(&failure, BudgetKind::SubstitutionBytes));
                assert_eq!(failure.code(), "KAMINIDE_BUDGET_EXCEEDED");
            }
        }
    }
}

#[test]
fn background_resource_aggregate_rejects_the_candidate_and_keeps_the_previous_revision() {
    let html = format!(r#"<div style="background-image: url({PNG})"></div>"#);
    let need = spent(&html, BudgetKind::ResourceBytes);
    assert!(need > 0, "background facts load before publication");
    for (limit, ok) in [(need - 1, false), (need, true), (need + 1, true)] {
        let built = build(&html, BudgetKind::ResourceBytes, limit);
        assert_eq!(built.is_ok(), ok);
        if let Err(failure) = built {
            assert!(revision_exceeded(&failure, BudgetKind::ResourceBytes));
        }
    }
    let mut doc = build("<p>a</p>", BudgetKind::ResourceBytes, need - 1).unwrap();
    let salt = doc.salt();
    let failure = doc.update(&html, "").unwrap_err();
    assert!(revision_exceeded(&failure, BudgetKind::ResourceBytes));
    assert_eq!(failure.report.retained_revision, Some(salt));
    assert_eq!(doc.salt(), salt);
    // Новая попытка — новая ревизия со своим work, а не продолжение исчерпанной.
    assert_eq!(doc.revision_budget().used(BudgetKind::ResourceBytes), 0);
}
