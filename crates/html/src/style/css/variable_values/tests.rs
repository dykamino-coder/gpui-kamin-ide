//! Verify custom-property computation independently of painting.

use super::super::parse_decls;
use super::{compute, has_var, substitute};
use std::collections::HashMap;

#[test]
fn variable_names_are_not_unescaped_twice() {
    let raw = parse_decls(r"--a\\31: green; --b: var(--a\\31)");
    let values = compute(&raw, &HashMap::new(), &HashMap::new(), |_, _| true);
    assert_eq!(values["--b"].trim(), "green");
}

#[test]
fn substituted_wide_keywords_apply_to_the_custom_property() {
    let parent = parse_decls("--a: green");
    let values = compute(
        &parse_decls("--a: var(--missing,unset)"),
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
        &parse_decls("--a: var(--missing,initial)"),
        &parent,
        &registered,
        |_, _| true,
    );
    assert!(!values.contains_key("--a"));
}

#[test]
fn inherited_variables_are_frozen_before_child_overrides() {
    let parent = compute(
        &parse_decls("--a: green; --b: var(--a)"),
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
        &parse_decls("--a: var(--b,red); --b: var(--a,red); --c: var(--a,green)"),
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
        &parse_decls("--a: green; --b: var(--a,var(--b))"),
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
    let parent = parse_decls("--a: red");
    for value in ["--a: var(--missing)", "--a: var(--a,blue)"] {
        let values = compute(&parse_decls(value), &parent, &registered, |_, _| true);
        assert!(!values.contains_key("--a"));
    }
}
