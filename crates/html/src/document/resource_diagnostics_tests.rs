//! Пределы ресурсов, подстановок и отмены заданий приходят в журнал документа одной
//! агрегированной записью со своими code, severity, scope и фазой.

use super::*;
use crate::budget::{BudgetKind, BudgetScope, DocumentBudget, DocumentLimits, ResourceGovernor};
use crate::diagnostics::{Code, Diagnostic, Measure, Phase, Severity};
use crate::resources::images::types::DecodeOptions;
use crate::resources::loader::LoadError;

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYPj/HwADAgH/eL9GtQAAAABJRU5ErkJggg==";

fn state(kind: BudgetKind, limit: u64) -> std::rc::Rc<state::DocState> {
    let doc = state::DocState::new(Default::default());
    let limits = DocumentLimits::default().lower(kind, limit).unwrap();
    let budget = DocumentBudget::new(limits, ResourceGovernor::default()).unwrap();
    doc.admit_revision(budget.revision());
    doc
}

fn only(doc: &state::DocState) -> Diagnostic {
    let (records, suppressed) = doc.take_diagnostics();
    let [record] = records.as_slice() else {
        panic!("{records:?}")
    };
    assert_eq!(suppressed, 0);
    *record
}

#[test]
fn image_budget_refusals_are_one_record_per_key_in_the_document_scope() {
    let doc = state(BudgetKind::ResourceBytes, 1);
    for _ in 0..3 {
        let mut images = doc.images.borrow_mut();
        images.begin_generation();
        assert!(
            images
                .resolve_image(PNG, DecodeOptions::default())
                .is_none()
        );
    }
    let record = only(&doc);
    assert_eq!(
        (
            record.code,
            record.severity,
            record.phase,
            record.occurrences
        ),
        (Code::BudgetExceeded, Severity::Error, Phase::Resource, 3)
    );
    assert_eq!(record.scope, FailureScope::Document);
    assert!(matches!(record.measure, Some(Measure::Budget(e))
        if e.kind == BudgetKind::ResourceBytes && e.scope == BudgetScope::Revision));
    let embedded = state(BudgetKind::PendingTasks, 0);
    embedded.traversal.set_embedded();
    assert!(
        embedded
            .images
            .borrow_mut()
            .resolve_image(PNG, DecodeOptions::default())
            .is_none()
    );
    let record = only(&embedded);
    assert_eq!(
        (record.scope, record.severity),
        (FailureScope::EmbeddedDocument, Severity::Warning)
    );
}

#[test]
fn cancelled_stale_jobs_are_info_telemetry() {
    let doc = state(BudgetKind::Documents, 1);
    let mut images = doc.images.borrow_mut();
    let (url, _) = images.resolve(PNG);
    images.request(crate::resources::images::store::ImageRequest {
        url,
        options: DecodeOptions::default(),
    });
    let jobs = images.take_jobs();
    images.begin_generation();
    let services = images.services().clone();
    assert!(images.apply_completed(jobs[0].run(&services)).is_none());
    drop(images);
    let record = only(&doc);
    assert_eq!(
        (record.code, record.severity, record.phase),
        (Code::Cancelled, Severity::Info, Phase::Resource)
    );
}

#[test]
fn synchronous_load_limits_and_substitution_limits_reach_the_journal() {
    let doc = state(BudgetKind::Documents, 1);
    doc.record_load_failure(&LoadError::TooLarge { limit: 7 });
    doc.record_load_failure(&LoadError::NotFound);
    let record = only(&doc);
    assert_eq!(
        (record.code, record.phase, record.measure),
        (
            Code::ResourceLimit,
            Phase::Resource,
            Some(Measure::Ceiling { limit: 7 })
        )
    );
    let long = "x".repeat(1 << 20);
    let html = format!(
        r#"<style>.a {{ --a: {long}; color: var(--a) }}</style><i class=a></i><i class=a></i>"#
    );
    let built = Document::builder(&html, "").build().unwrap();
    let batch = built.take_diagnostics();
    let [record] = batch.records.as_slice() else {
        panic!("{:?}", batch.records)
    };
    assert_eq!(
        (
            record.code,
            record.severity,
            record.phase,
            record.occurrences
        ),
        (Code::SubstitutionLimit, Severity::Warning, Phase::Style, 2)
    );
    assert!(matches!(record.measure, Some(Measure::Length { limit, .. }) if limit == 1 << 20));
}
