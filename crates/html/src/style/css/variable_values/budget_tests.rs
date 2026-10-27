//! Бюджеты подстановки: работа DAG пользовательских свойств элемента, aggregate ревизии
//! на границе и отсутствие расхода для значений без `var()`.

use super::super::VarMap;
use super::{SubstitutionLimit, Unsubstituted, Work, compute, substitute};
use crate::budget::{
    BudgetError, BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor, RevisionBudget,
};
use std::collections::HashMap;

fn revision(limit: u64) -> RevisionBudget {
    let limits = DocumentLimits::default()
        .lower(BudgetKind::SubstitutionBytes, limit)
        .unwrap();
    DocumentBudget::new(limits, ResourceGovernor::default())
        .unwrap()
        .revision()
}

/// `var(--a)` при `--a: green`: 8 × 8 байт сканирования и 7 байт результата.
const GREEN: u64 = 71;

#[test]
fn revision_aggregate_boundary_makes_the_value_invalid_and_the_revision_terminal() {
    for (limit, ok) in [(GREEN - 1, false), (GREEN, true), (GREEN + 1, true)] {
        let revision = revision(limit);
        let result = substitute(
            "var(--a)",
            &mut |_| Some("green".into()),
            &Work::new(&revision),
        );
        assert_eq!(result.is_ok(), ok);
        assert_eq!(revision.exhausted().is_some(), !ok);
        if !ok {
            assert!(matches!(
                result,
                Err(Unsubstituted::Limit(SubstitutionLimit::Revision(
                    BudgetError::Exceeded(e)
                ))) if e.kind == BudgetKind::SubstitutionBytes && e.increment == 1
            ));
        }
        // Списанная работа не возвращается: отказ на последнем байте результата.
        let used = revision.used(BudgetKind::SubstitutionBytes);
        assert_eq!(used, if ok { GREEN } else { GREEN - 1 });
    }
}

#[test]
fn values_without_var_neither_substitute_nor_spend() {
    let revision = revision(0);
    let work = Work::new(&revision);
    assert_eq!(substitute("green", &mut |_| None, &work).as_deref(), Ok("green"));
    assert_eq!(revision.used(BudgetKind::SubstitutionBytes), 0);
    assert!(revision.exhausted().is_none());
}

#[test]
fn many_properties_of_one_element_share_the_dag_work_budget() {
    // Каждое `--bNN: var(--a)` укладывается в предел значения (1 МиБ результата),
    // но DAG элемента делит 32 МиБ работы: 31 × (64 + 1 МиБ) помещается, 32-е — нет.
    let a = "x".repeat((1 << 20) - 2);
    for (count, valid) in [(31, 31), (32, 31), (33, 31)] {
        let mut raw = VarMap::from([("--a".to_string(), a.clone())]);
        for n in 0..count {
            raw.insert(format!("--b{n:02}"), "var(--a)".into());
        }
        let revision = RevisionBudget::standalone();
        let values = compute(&raw, &VarMap::new(), &HashMap::new(), |_, _| true, &revision);
        let resolved = (0..count)
            .filter(|n| values.contains_key(&format!("--b{n:02}")))
            .count();
        assert_eq!(resolved, valid);
        // Детерминированный порядок: негодны всегда последние по имени.
        assert!((0..valid).all(|n| values.contains_key(&format!("--b{n:02}"))));
        assert!(revision.exhausted().is_none(), "a per-element limit is recoverable");
    }
}
