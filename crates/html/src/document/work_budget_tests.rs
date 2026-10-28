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
    assert_eq!(
        doc.update("<p>a</p>", "").unwrap().status,
        RevisionStatus::Unchanged
    );
    assert!(doc.last_failure().is_none());
}

#[test]
fn generated_value_refusal_stays_terminal_after_diagnostics_and_unchanged_update() {
    let mut doc = Document::builder("<p>KEEP</p>", "").build().unwrap();
    let failure = DocumentFailure::new(FailureReason::Allocation);
    doc.revision_budget().reject(failure);
    let observed = doc.last_failure().unwrap();
    assert_eq!(observed.reason, FailureReason::Allocation);
    assert_eq!(doc.report().status, RevisionStatus::Rejected);
    assert_eq!(doc.take_diagnostics().terminal, Some(observed));
    assert_eq!(doc.take_diagnostics().terminal, Some(observed));
    assert_eq!(doc.update("<p>KEEP</p>", ""), Err(observed));
    doc.update("<p>RECOVER</p>", "").unwrap();
    assert!(doc.last_failure().is_none());
}

#[test]
fn generated_counter_exhaustion_uses_the_build_failure_channel() {
    let html = "<style>@counter-style x { system: symbolic; symbols: 'x' }</style><p>KEEP</p>";
    let mut doc = Document::builder(html, "").build().unwrap();
    let revision = doc.revision_budget();
    let left = revision
        .document()
        .limits()
        .limit(BudgetKind::SubstitutionBytes)
        - revision.used(BudgetKind::SubstitutionBytes);
    revision
        .consume(BudgetKind::SubstitutionBytes, left)
        .unwrap();
    crate::style::generated::counter_style_rules::custom_repr(
        1501,
        "x",
        Default::default(),
        doc.state.values(),
    );
    let failure = doc
        .last_failure()
        .expect("generated value failure is terminal");
    assert!(revision_exceeded(&failure, BudgetKind::SubstitutionBytes));
    assert_eq!(doc.take_diagnostics().terminal, Some(failure));
    assert_eq!(doc.update(html, ""), Err(failure));
    doc.update("<p>RECOVER</p>", "").unwrap();
    assert!(doc.last_failure().is_none());
}
