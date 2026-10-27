//! Журнал: предел записей ±1, агрегация, подавленные, выдача; коды и представление.

use super::*;
use crate::budget::{BudgetKind, BudgetScope, BudgetUnit};

fn at(row: u32) -> Diagnostic {
    Diagnostic {
        code: Code::XmlSyntax,
        severity: Severity::Error,
        scope: Scope::Document,
        phase: Phase::Parse,
        measure: None,
        location: Some(Location::Source { row, column: 1 }),
        occurrences: 1,
    }
}

fn filled(distinct: u32) -> DiagnosticLog {
    let mut log = DiagnosticLog::default();
    for row in 0..distinct {
        log.record(at(row));
    }
    log
}

#[test]
fn record_limit_is_exact_at_minus_one_zero_and_plus_one() {
    for (distinct, kept, suppressed) in [
        (MAX_RECORDS - 1, MAX_RECORDS - 1, 0),
        (MAX_RECORDS, MAX_RECORDS, 0),
        (MAX_RECORDS + 1, MAX_RECORDS, 1),
    ] {
        let (records, lost) = filled(distinct as u32).snapshot();
        assert_eq!((records.len(), lost), (kept, suppressed));
    }
}

#[test]
fn repeats_aggregate_by_key_and_counters_saturate() {
    let mut log = DiagnosticLog::default();
    for _ in 0..1_000_000 {
        log.record(at(7));
    }
    let (records, suppressed) = log.snapshot();
    assert_eq!(
        (records.len(), records[0].occurrences, suppressed),
        (1, 1_000_000, 0)
    );
    log.record(Diagnostic {
        occurrences: u32::MAX,
        ..at(7)
    });
    assert_eq!(log.snapshot().0[0].occurrences, u32::MAX);
    let mut full = filled(MAX_RECORDS as u32);
    full.suppress(u64::MAX);
    full.record(at(u32::MAX));
    assert_eq!(full.snapshot().1, u64::MAX);
}

#[test]
fn without_a_subscriber_the_log_neither_grows_nor_reallocates() {
    let mut log = DiagnosticLog::default();
    let reserved = log.capacity();
    assert!(reserved >= MAX_RECORDS);
    for row in 0..1_000_000 {
        log.record(at(row));
    }
    assert_eq!(log.capacity(), reserved);
    let (records, suppressed) = log.snapshot();
    assert_eq!(records.len(), MAX_RECORDS);
    assert_eq!(suppressed, 1_000_000 - MAX_RECORDS as u64);
}

#[test]
fn take_delivers_each_key_once_and_the_suppressed_delta() {
    let mut log = filled(2);
    assert_eq!(log.take(), (vec![at(0), at(1)], 0));
    log.record(at(0));
    assert_eq!(log.take(), (vec![], 0));
    assert_eq!(log.snapshot().0[0].occurrences, 2);
    for row in 2..=MAX_RECORDS as u32 + 1 {
        log.record(at(row));
    }
    let (fresh, suppressed) = log.take();
    assert_eq!((fresh.len(), suppressed), (MAX_RECORDS - 2, 2));
    assert_eq!(log.take(), (vec![], 0));
}

#[test]
fn codes_severity_scope_and_phase_come_from_typed_outcomes() {
    let limit = TraversalLimit {
        scope: Scope::EmbeddedDocument,
        traversal: TraversalKind::Layout,
        limit: 1026,
        attempted: 1027,
    };
    let record = Diagnostic::from(limit);
    assert_eq!(record.code.as_str(), "KAMINIDE_LIMIT_TRAVERSAL_DEPTH");
    assert_eq!(record.code.as_str(), limit.code());
    assert_eq!(record.severity, Severity::Warning);
    assert_eq!(record.scope, Scope::EmbeddedDocument);
    assert_eq!(record.phase, Phase::Traversal(TraversalKind::Layout));
    assert_eq!(
        record.measure,
        Some(Measure::Depth {
            limit: 1026,
            attempted: 1027
        })
    );
    let xml = DocumentFailure::new(FailureReason::XmlSyntax { row: 3, column: 9 });
    let record = Diagnostic::from(&xml);
    assert_eq!(
        (record.code.as_str(), xml.code()),
        ("KAMINIDE_XML_SYNTAX", "KAMINIDE_XML_SYNTAX")
    );
    assert_eq!(
        (record.severity, record.phase),
        (Severity::Error, Phase::Parse)
    );
    assert_eq!(
        record.location,
        Some(Location::Source { row: 3, column: 9 })
    );
    let exceeded = BudgetExceeded {
        scope: BudgetScope::Revision,
        kind: BudgetKind::SourceBytes,
        unit: BudgetUnit::Bytes,
        limit: 10,
        consumed: 4,
        increment: 7,
        overflow: false,
    };
    let budget = DocumentFailure::new(FailureReason::Budget(BudgetError::Exceeded(exceeded)));
    let record = Diagnostic::from(&budget);
    assert_eq!(record.code.as_str(), "KAMINIDE_BUDGET_EXCEEDED");
    assert_eq!((record.phase, record.location), (Phase::Admission, None));
    assert_eq!(record.measure, Some(Measure::Budget(exceeded)));
    let invariant = DocumentFailure::new(FailureReason::Budget(BudgetError::ForeignGovernor));
    assert_eq!(invariant.code(), "KAMINIDE_BUDGET_INVARIANT");
    assert_eq!(Diagnostic::from(&invariant).measure, None);
    assert!(Severity::Error > Severity::Warning);
}

#[test]
fn console_lines_render_typed_fields_terminal_first_and_suppressed_total() {
    let limit = TraversalLimit {
        scope: Scope::Document,
        traversal: TraversalKind::BoxBuild,
        limit: 2052,
        attempted: 2053,
    };
    let batch = DiagnosticBatch {
        revision: Some(0xab),
        records: vec![Diagnostic {
            occurrences: 3,
            ..limit.into()
        }],
        suppressed: 5,
        terminal: Some(DocumentFailure::new(FailureReason::XmlSyntax {
            row: 1,
            column: 2,
        })),
    };
    let mut out = String::new();
    console::write_batch(&mut out, "t|r|", &batch);
    assert_eq!(
        out,
        "t|r|error KAMINIDE_XML_SYNTAX scope=document phase=parse at=1:2 count=1 \
         revision=00000000000000ab\n\
         t|r|warning KAMINIDE_LIMIT_TRAVERSAL_DEPTH scope=document phase=traversal-BoxBuild \
         limit=2052 attempted=2053 unit=depth count=3 revision=00000000000000ab\n\
         t|r|warning KAMINIDE_DIAGNOSTICS_SUPPRESSED count=5 revision=00000000000000ab\n"
    );
    assert!(DiagnosticBatch::default().is_empty());
    assert!(!batch.is_empty());
}

#[test]
fn display_status_names_the_first_cause_and_groups_the_rest() {
    let limit = TraversalLimit {
        scope: Scope::Document,
        traversal: TraversalKind::BoxBuild,
        limit: 2052,
        attempted: 2053,
    };
    let degraded = |problems, suppressed| DisplayStatus::Degraded {
        first_limit: Some(limit),
        embedded_failure: None,
        problems,
        suppressed,
    };
    assert_eq!(degraded(1, 0).message(), Some(limit.to_string()));
    let grouped = degraded(3, 4).message().unwrap();
    assert!(grouped.starts_with(&limit.to_string()) && grouped.contains("Ещё проблем: 6"));
    assert_eq!(DisplayStatus::Complete.message(), None);
    let failure = DocumentFailure::new(FailureReason::XmlSyntax { row: 1, column: 1 });
    let failed = DisplayStatus::Failed(failure);
    assert!(failed.is_failure() && !degraded(1, 0).is_failure());
    assert!(
        failed
            .message()
            .unwrap()
            .starts_with("Документ не отображён")
    );
}
