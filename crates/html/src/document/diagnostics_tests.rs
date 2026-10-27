//! Выдача диагностик документа: исходы Complete/Degraded/Failed, отказ update
//! вне очереди, вложенные документы и штатно игнорируемый CSS.

use super::*;
use crate::diagnostics::{Diagnostic, DisplayStatus, Measure, Phase, Severity};

fn deep(depth: usize) -> String {
    format!(
        "{}text{}",
        "<div>".repeat(depth - 3),
        "</div>".repeat(depth - 3)
    )
}

#[test]
fn ignored_css_and_unsupported_features_are_not_diagnostics() {
    let css = "p { color: nonsense; unknown-property: 1; width: calc(; } \
               @unknown-rule { p { color: red } } p { color: green !important }";
    let html = format!("<style>{css}</style><p style='margin: wrong'>x</p>");
    let doc = Document::builder(&html, css).build().unwrap();
    assert!(doc.take_diagnostics().records.is_empty());
    assert!(doc.diagnostics().is_empty());
    assert_eq!(doc.report().status, RevisionStatus::Complete);
    assert_eq!(doc.display_status(), DisplayStatus::Complete);
    assert!(doc.status_message().is_none());
}

#[test]
fn box_limit_is_one_aggregated_warning_delivered_once() {
    let doc = Document::builder(&deep(2053), "").build().unwrap();
    let batch = doc.take_diagnostics();
    assert_eq!(batch.revision, Some(doc.salt()));
    assert_eq!((batch.suppressed, batch.terminal), (0, None));
    let [record] = batch.records.as_slice() else {
        panic!("{:?}", batch.records)
    };
    assert_eq!(record.severity, Severity::Warning);
    assert_eq!(record.phase, Phase::Traversal(TraversalKind::BoxBuild));
    assert_eq!(
        record.measure,
        Some(Measure::Depth {
            limit: 2052,
            attempted: 2053
        })
    );
    assert!(doc.take_diagnostics().records.is_empty());
    assert_eq!(doc.diagnostics().records, batch.records);
    assert_eq!(doc.report().status, RevisionStatus::Degraded);
    assert!(matches!(
        doc.display_status(),
        DisplayStatus::Degraded {
            problems: 1,
            suppressed: 0,
            ..
        }
    ));
}

#[test]
fn rejected_update_is_terminal_outside_the_queue_until_the_next_commit() {
    let mut doc = Document::builder("<svg/>", "")
        .path(std::path::Path::new("a.svg"))
        .build()
        .unwrap();
    let failure = doc.update("<svg>", "").unwrap_err();
    for _ in 0..2 {
        let batch = doc.take_diagnostics();
        assert_eq!(batch.terminal, Some(failure));
        assert!(batch.records.is_empty());
    }
    assert_eq!(doc.display_status(), DisplayStatus::Stale(failure));
    assert_eq!(
        Diagnostic::from(&failure).code.as_str(),
        "KAMINIDE_XML_SYNTAX"
    );
    doc.update("<svg><g/></svg>", "").unwrap();
    assert!(doc.take_diagnostics().is_empty());
    assert_eq!(doc.display_status(), DisplayStatus::Complete);
}

#[test]
fn first_load_failure_is_a_compact_failed_outcome() {
    let failure = Document::builder("<svg>", "")
        .path(std::path::Path::new("a.svg"))
        .build()
        .err()
        .unwrap();
    assert_eq!(failure.report.status, RevisionStatus::Rejected);
    assert_eq!(failure.report.retained_revision, None);
    let status = DisplayStatus::Failed(failure);
    assert!(status.is_failure());
    assert!(
        status
            .message()
            .unwrap()
            .starts_with("Документ не отображён")
    );
}

#[test]
fn embedded_records_reach_the_owner_with_their_scope_and_bounded_counts() {
    let root = state::DocState::new(Default::default());
    let failure = DocumentFailure {
        scope: FailureScope::EmbeddedDocument,
        ..DocumentFailure::new(FailureReason::XmlSyntax { row: 1, column: 1 })
    };
    for _frame in 0..3 {
        // Кадр пересобирает вложенный документ: новое состояние, тот же ключ.
        root.begin_frame();
        let child = state::DocState::new(Default::default());
        let grandchild = state::DocState::new(Default::default());
        grandchild.record_diagnostic((&failure).into());
        child.observe_child(grandchild);
        root.observe_child(child);
        let _ = root.take_diagnostics();
    }
    let (records, suppressed) = root.diagnostics_snapshot();
    let [record] = records.as_slice() else {
        panic!("{records:?}")
    };
    assert_eq!(
        (record.scope, record.occurrences, suppressed),
        (FailureScope::EmbeddedDocument, 3, 0)
    );
    assert_eq!(record.severity, Severity::Error);
}
