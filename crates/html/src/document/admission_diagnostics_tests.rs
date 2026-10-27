//! Отказы допуска входа (парсер, CSS, XML, выделения) приходят в канал диагностик
//! ровно одной записью со своими code, scope и фазой.

use super::*;
use crate::budget::{BudgetError, BudgetKind, DocumentLimits, ResourceGovernor};
use crate::diagnostics::{Code, Diagnostic, Measure, Phase, Severity};

fn limits(kind: BudgetKind, limit: u64) -> DocumentLimits {
    DocumentLimits::default().lower(kind, limit).unwrap()
}

#[test]
fn first_load_and_update_admission_failures_are_one_terminal_record() {
    let css = "<style>p {color:red} a {color:blue}</style><p>KEEP</p>";
    let governor = ResourceGovernor::default();
    let plain = Document::builder("<p>a</p>", "").build().unwrap();
    let rules = plain.revision_budget().used(BudgetKind::CssRules);
    let failure = Document::builder(css, "")
        .limits(limits(BudgetKind::CssRules, rules))
        .governor(&governor)
        .build()
        .err()
        .unwrap();
    let record = Diagnostic::from(&failure);
    assert_eq!(
        (record.code, record.scope, record.phase, record.severity),
        (
            Code::BudgetExceeded,
            FailureScope::Document,
            Phase::Admission,
            Severity::Error
        )
    );
    assert!(matches!(record.measure, Some(Measure::Budget(e)) if e.kind == BudgetKind::CssRules));
    let mut doc = Document::builder("<p>a</p>", "")
        .limits(limits(BudgetKind::CssRules, rules))
        .governor(&governor)
        .build()
        .unwrap();
    let failure = doc.update(css, "").unwrap_err();
    let batch = doc.take_diagnostics();
    assert!(batch.records.is_empty(), "{:?}", batch.records);
    assert_eq!(batch.terminal, Some(failure));
}

#[test]
fn every_embedded_admission_reason_is_one_record_with_its_scope_and_phase() {
    let exceeded = Document::builder("<p>KEEP</p>", "")
        .limits(limits(BudgetKind::SourceBytes, 1))
        .build()
        .err()
        .unwrap()
        .reason;
    for (reason, code, phase) in [
        (exceeded, Code::BudgetExceeded, Phase::Admission),
        (
            FailureReason::Budget(BudgetError::ForeignGovernor),
            Code::BudgetInvariant,
            Phase::Admission,
        ),
        (
            FailureReason::Allocation,
            Code::AllocationFailed,
            Phase::Admission,
        ),
        (
            FailureReason::CapacityOverflow,
            Code::CapacityOverflow,
            Phase::Admission,
        ),
        (
            FailureReason::XmlSyntax { row: 2, column: 3 },
            Code::XmlSyntax,
            Phase::Parse,
        ),
    ] {
        let failure = DocumentFailure {
            scope: FailureScope::EmbeddedDocument,
            ..DocumentFailure::new(reason)
        };
        assert_eq!(failure.code(), code.as_str());
        let owner = state::DocState::new(Default::default());
        owner.record_diagnostic((&failure).into());
        let (records, suppressed) = owner.take_diagnostics();
        let [record] = records.as_slice() else {
            panic!("{records:?}")
        };
        assert_eq!(
            (record.code, record.scope, record.phase, suppressed),
            (code, FailureScope::EmbeddedDocument, phase, 0)
        );
    }
}
