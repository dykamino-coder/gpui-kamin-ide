//! Тесты вычисления пользовательских свойств (`compute`/`substitute`) без
//! отрисовки: экраны в именах, общие CSS-слова через `var()`, `@property`.

use super::super::{Registered, VarMap, parse_decls};
use super::{Work, has_var};
use crate::budget::RevisionBudget;
use std::collections::HashMap;

/// Подстановка одного значения в стандартном профиле отдельной ревизии.
pub(super) fn substitute(
    raw: &str,
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Option<String> {
    let revision = RevisionBudget::standalone();
    super::substitute(raw, lookup, &Work::new(&revision)).ok()
}

pub(super) fn compute(
    raw: &VarMap,
    parent: &VarMap,
    registered: &HashMap<String, Registered>,
    accepts: impl Fn(&str, &str) -> bool,
) -> VarMap {
    super::compute(raw, parent, registered, accepts, &RevisionBudget::standalone())
}

/// Custom-property declarations of a block as a substitution map.
fn vars(css: &str) -> VarMap {
    parse_decls(css)
        .iter()
        .map(|d| (d.name.clone(), d.value.clone()))
        .collect()
}

#[test]
fn variable_names_are_not_unescaped_twice() {
    for css in [
        r"--a\\31: green; --b: var(--a\\31)",
        r"--a\ b: green; --b: var(--a\ b)",
        r"--a\ b: red; --b: var(--a b, green)",
    ] {
        let values = compute(&vars(css), &HashMap::new(), &HashMap::new(), |_, _| true);
        assert_eq!(values["--b"].trim(), "green", "{css}");
    }
}

#[test]
fn substituted_wide_keywords_apply_to_the_custom_property() {
    let parent = vars("--a: green");
    let values = compute(
        &vars("--a: var(--missing,unset)"),
        &parent,
        &HashMap::new(),
        |_, _| true,
    );
    assert_eq!(values["--a"].trim(), "green");
    let reg = super::super::Registered {
        syntax: "*".into(),
        inherits: true,
        initial: None,
    };
    let registered = HashMap::from([("--a".to_string(), reg)]);
    let values = compute(
        &vars("--a: var(--missing,initial)"),
        &parent,
        &registered,
        |_, _| true,
    );
    assert!(!values.contains_key("--a"));
}

#[test]
fn inherited_variables_are_frozen_before_child_overrides() {
    let parent = compute(
        &vars("--a: green; --b: var(--a)"),
        &HashMap::new(),
        &HashMap::new(),
        |_, _| true,
    );
    let mut child = parent.clone();
    child.insert("--a".into(), "red".into());
    let child = compute(&child, &parent, &HashMap::new(), |_, _| true);
    assert_eq!(child["--b"].trim(), "green");
}

#[test]
fn cycles_invalidate_participants_but_dependents_can_use_fallbacks() {
    let values = compute(
        &vars("--a: var(--b,red); --b: var(--a,red); --c: var(--a,green)"),
        &HashMap::new(),
        &HashMap::new(),
        |_, _| true,
    );
    assert!(!values.contains_key("--a"));
    assert!(!values.contains_key("--b"));
    assert_eq!(values["--c"].trim(), "green");
}

#[test]
fn unused_fallbacks_do_not_create_cycles() {
    let values = compute(
        &vars("--a: green; --b: var(--a,var(--b))"),
        &HashMap::new(),
        &HashMap::new(),
        |_, _| true,
    );
    assert_eq!(values["--b"].trim(), "green");
}

#[test]
fn quoted_functions_and_adjacent_tokens_remain_distinct() {
    assert!(has_var("rgb(var(--a),0,0)"));
    assert!(!has_var("\"var(--a)\""));
    assert!(!has_var("/*var(--a)*/green"));
    let mut lookup = |_: &str| Some("20".to_string());
    assert_eq!(
        substitute("var(--a)px", &mut lookup).as_deref(),
        Some(" 20 px")
    );
    assert_eq!(
        substitute("\"var(--a)\"", &mut lookup).as_deref(),
        Some("\"var(--a)\"")
    );
    assert_eq!(
        substitute("var(--a,)", &mut |_| None).as_deref(),
        Some("  ")
    );
    assert_eq!(substitute("var(--a)", &mut |_| None), None);
}

#[test]
fn universal_registrations_keep_failed_substitutions_invalid() {
    let reg = super::super::Registered {
        syntax: "*".into(),
        inherits: true,
        initial: Some("green".into()),
    };
    let registered = HashMap::from([("--a".to_string(), reg)]);
    let parent = vars("--a: red");
    for value in ["--a: var(--missing)", "--a: var(--a,blue)"] {
        let values = compute(&vars(value), &parent, &registered, |_, _| true);
        assert!(!values.contains_key("--a"));
    }
}
