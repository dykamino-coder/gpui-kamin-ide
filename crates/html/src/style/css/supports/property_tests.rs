//! Capability tests for property recognition before variable substitution.

#[test]
fn variables_defer_values_only_for_recognized_properties() {
    for condition in [
        "(definitely-not-a-property: var(--x))",
        "(definitely-not-a-property: initial)",
        "(definitely-not-a-property: red)",
    ] {
        assert!(!crate::style::css::supports(condition), "{condition}");
    }
    for condition in [
        "(color: var(--x))",
        "(COLOR: var(--x))",
        r"(c\6flor: var(--x))",
        "(--custom: var(--x))",
    ] {
        assert!(crate::style::css::supports(condition), "{condition}");
    }
}
