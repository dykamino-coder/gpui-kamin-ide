//! Selector capability queries follow the parser used by the actual stylesheet cascade.

use super::supports;

#[test]
fn unsupported_pseudo_elements_are_not_advertised_as_capabilities() {
    for selector in [
        "::picker(select)",
        "select::picker-icon",
        "input::-webkit-slider-thumb",
        "input::-webkit-slider-runnable-track",
        "div::unknown-pseudo",
    ] {
        assert!(!supports(&format!("selector({selector})")), "{selector}");
    }
    for selector in [
        "div::before",
        "details::details-content",
        ".x:hover",
        "&",
        "& > p",
    ] {
        assert!(supports(&format!("selector({selector})")), "{selector}");
    }
}

#[test]
fn nesting_and_pseudo_names_inside_attribute_strings_are_preserved() {
    for selector in [
        r#"[data-value="&"]"#,
        r#"[data-value="::picker(select)"]"#,
        r#"[data-value="::-webkit-slider-thumb"]"#,
        r#"[data-value="::picker-icon"]"#,
    ] {
        assert!(supports(&format!("selector({selector})")), "{selector}");
    }
    assert!(!supports("selector(div, span)"));
}
