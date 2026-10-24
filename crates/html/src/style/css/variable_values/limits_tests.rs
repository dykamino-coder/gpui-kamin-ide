//! Пределы подстановки: ациклические графы, выбранные fallback и отказ без
//! усечения результата; негодное значение сохраняет обычную семантику var().

use super::super::{Registered, VarMap};
use super::{compute, substitute};
use std::collections::HashMap;

#[test]
fn exponential_dag_becomes_invalid_and_dependents_can_use_fallback() {
    let mut raw = VarMap::from([("--v0".into(), "x".into())]);
    for n in 1..=22 {
        raw.insert(
            format!("--v{n}"),
            format!("var(--v{})var(--v{})", n - 1, n - 1),
        );
    }
    raw.insert("--safe".into(), "var(--v22,green)".into());
    let values = compute(&raw, &VarMap::new(), &HashMap::new(), |_, _| true);
    assert_eq!(values["--v17"].matches('x').count(), 1 << 17);
    for n in 18..=22 {
        assert!(!values.contains_key(&format!("--v{n}")));
    }
    assert_eq!(values["--safe"].trim(), "green");
}

#[test]
fn long_linear_graph_is_not_rejected_by_a_depth_limit() {
    let mut raw = VarMap::from([("--v0".into(), "green".into())]);
    for n in 1..=300 {
        raw.insert(format!("--v{n}"), format!("var(--v{})", n - 1));
    }
    let values = compute(&raw, &VarMap::new(), &HashMap::new(), |_, _| true);
    assert_eq!(values["--v300"].trim(), "green");
}

#[test]
fn output_limit_counts_utf8_bytes_and_never_returns_a_prefix() {
    let limit = 1 << 20;
    let value = "é".repeat((limit - 2) / 2);
    let result = substitute("var(--x)", &mut |_| Some(value.clone())).unwrap();
    assert_eq!(result.len(), limit);
    assert_eq!(result.trim(), value);
    assert!(substitute("var(--x)y", &mut |_| Some(value.clone())).is_none());
    assert!(substitute(&"x".repeat(limit + 1), &mut |_| None).is_none());
}

#[test]
fn selected_fallbacks_share_work_budget_but_unused_ones_are_not_expanded() {
    let mut raw = "green".to_string();
    for _ in 0..100 {
        raw = format!("var(--missing,{}{raw})", " ".repeat(4096));
    }
    // И исходная строка, и полный результат меньше предела длины; отказ
    // обусловлен повторным сканированием вложенных запасных значений.
    assert!(raw.len() < 1 << 20);
    assert!(substitute(&raw, &mut |_| None).is_none());
    let unused = format!("var(--present,{raw})");
    let result = substitute(&unused, &mut |_| Some("green".into())).unwrap();
    assert_eq!(result.trim(), "green");
}

#[test]
fn oversized_registered_value_uses_registered_computed_failure_rules() {
    let raw = VarMap::from([
        ("--color".into(), "var(--source)".into()),
        ("--source".into(), "x".repeat((1 << 20) + 1)),
    ]);
    let registered = HashMap::from([(
        "--color".into(),
        Registered {
            syntax: "<color>".into(),
            inherits: false,
            initial: Some("green".into()),
        },
    )]);
    let values = compute(&raw, &VarMap::new(), &registered, |_, v| v == "green");
    assert_eq!(values["--color"], "green");
    assert!(!values.contains_key("--source"));
}
