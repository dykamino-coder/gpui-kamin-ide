//! Сохранённый DOM допускается в бюджет кандидата; отказ не расходует бюджет
//! опубликованной ревизии и не оставляет в DOM контекст отвергнутой сборки.

use super::{state, tree};
use crate::budget::{BudgetError, BudgetKind, BudgetScope, DocumentLimits, ResourceGovernor};
use crate::document::{ApplyError, BuildReport, Document, FailureReason};
use std::rc::Rc;

const PAGE: &str = "<!doctype html><style>.on{color:red}[data-n]{color:blue}</style>\
    <p dir=auto>a</p>";

fn css_cost() -> u64 {
    Document::builder(PAGE, "")
        .build()
        .unwrap()
        .revision_budget()
        .used(BudgetKind::CssDeclarations)
}

fn limits() -> DocumentLimits {
    // Две полные ревизии помещаются по отдельности; история из двадцати — нет.
    DocumentLimits::default()
        .lower(BudgetKind::CssDeclarations, 2 * css_cost())
        .unwrap()
}

fn refused(result: Result<BuildReport, ApplyError>, kind: BudgetKind) {
    assert!(
        matches!(&result, Err(ApplyError::Rejected(failure))
            if matches!(failure.reason, FailureReason::Budget(BudgetError::Exceeded(e))
                if e.kind == kind && e.scope == BudgetScope::Revision)),
        "{result:?}"
    );
}

fn repeated(edited: &str) {
    let limits = limits();
    let governor = ResourceGovernor::default();
    let mut doc = Document::builder(PAGE, "")
        .limits(limits.clone())
        .governor(&governor)
        .build()
        .unwrap();
    let dom = doc.source_dom();
    for index in 0..20 {
        let html = if index % 2 == 0 { edited } else { PAGE };
        let records = doc.mutations(html).unwrap();
        let values = doc.state.values.clone();
        let result = doc.apply(&records, "");
        assert!(result.is_ok(), "revision {index}: {result:?}");
        assert!(!Rc::ptr_eq(&values, &doc.state.values), "full rebuild");
        assert!(Rc::ptr_eq(&dom, &doc.source_dom()), "same source DOM");
        let fresh = Document::builder(html, "")
            .limits(limits.clone())
            .governor(&governor)
            .build()
            .unwrap();
        assert_eq!(tree(&doc), tree(&fresh));
        assert_eq!(state(&doc), state(&fresh));
    }
    assert_eq!(
        doc.revision_budget().used(BudgetKind::CssDeclarations),
        css_cost()
    );
}

#[test]
fn repeated_class_rebuilds_use_the_candidate_css_budget() {
    repeated(&PAGE.replace("<p dir=auto>", "<p dir=auto class=on>"));
}

#[test]
fn repeated_text_rebuilds_use_the_candidate_css_budget() {
    // dir=auto читает исходный текст при каскаде, поэтому это полная сборка.
    repeated(&PAGE.replace(">a<", ">b<"));
}

#[test]
fn repeated_attribute_rebuilds_use_the_candidate_css_budget() {
    repeated(&PAGE.replace("<p dir=auto>", "<p dir=auto data-n=1>"));
}

#[test]
fn rebuilt_context_still_counts_the_live_source_dom() {
    use crate::dom::input::Admitted;
    let mut pending = vec![Document::builder(PAGE, "").build().unwrap().source_dom()];
    let mut count = 0;
    while let Some(node) = pending.pop() {
        count += 1;
        pending.extend(node.children.borrow().iter().cloned());
    }
    let mut doc = Document::builder(PAGE, "")
        .limits(limits().lower(BudgetKind::DomNodes, count).unwrap())
        .governor(&ResourceGovernor::default())
        .build()
        .unwrap();
    let html = PAGE.replace("<p dir=auto>", "<p dir=auto class=on>");
    doc.apply(&doc.mutations(&html).unwrap(), "").unwrap();
    let context = doc.state.values.input_context().unwrap();
    let failure = Admitted::with_context("<p>child</p>", false, context)
        .err()
        .expect("the source DOM already occupies the node limit");
    assert!(matches!(failure.reason,
        FailureReason::Budget(BudgetError::Exceeded(e)) if e.kind == BudgetKind::DomNodes));
}

#[test]
fn rejected_css_admission_preserves_the_committed_budget() {
    let mut doc = Document::builder(PAGE, "")
        .limits(limits())
        .governor(&ResourceGovernor::default())
        .build()
        .unwrap();
    let budget = doc.revision_budget();
    let used = budget.used(BudgetKind::CssDeclarations);
    let salt = doc.salt();
    let theme = format!("p{{{}}}", "color:red;".repeat((2 * used) as usize));
    for _ in 0..3 {
        refused(doc.apply(&[], &theme), BudgetKind::CssDeclarations);
        assert_eq!(doc.salt(), salt);
        assert_eq!(budget.used(BudgetKind::CssDeclarations), used);
        assert!(budget.failure().is_none());
        assert!(
            doc.apply(&[], "").is_ok(),
            "committed revision stays usable"
        );
    }
    let html = PAGE.replace("<p dir=auto>", "<p dir=auto class=on>");
    doc.apply(&doc.mutations(&html).unwrap(), "").unwrap();
}

#[test]
fn rejected_cascade_restores_the_source_admission_context() {
    let limits = limits().lower(BudgetKind::SubstitutionBytes, 1).unwrap();
    let mut doc = Document::builder(PAGE, "")
        .limits(limits)
        .governor(&ResourceGovernor::default())
        .build()
        .unwrap();
    let source = doc.state.values.traversal.take_source().unwrap();
    let budget = source.context().budget().clone();
    let agent_css = source.agent_css.get().0;
    doc.state.values.traversal.retain(source);
    let used = budget.used(BudgetKind::CssDeclarations);
    // Ранний допуск проходит; отказ происходит позже, при подстановке var().
    let theme = "p{--a:green;color:var(--a)}";
    refused(doc.apply(&[], theme), BudgetKind::SubstitutionBytes);
    let source = doc.state.values.traversal.take_source().unwrap();
    let restored = source.context().budget().clone();
    assert_eq!(source.agent_css.get().0, agent_css);
    doc.state.values.traversal.retain(source);
    restored.consume(BudgetKind::CssDeclarations, 1).unwrap();
    assert_eq!(budget.used(BudgetKind::CssDeclarations), used + 1);
    assert!(budget.failure().is_none());
    let html = PAGE.replace("<p dir=auto>", "<p dir=auto data-n=1>");
    doc.apply(&doc.mutations(&html).unwrap(), "").unwrap();
}
