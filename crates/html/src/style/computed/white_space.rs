//! Parse white-space without replacing inheritance or invalid values with normal.

use super::Computed;

pub(super) fn apply(style: &mut Computed, value: &str) {
    // CSS Cascade 4 section 7.3.2 inherits the parent's computed value.
    // These optional slots inherit during inline::inherit; explicit false
    // would instead override every component with normal's behavior.
    if value.eq_ignore_ascii_case("inherit") {
        style.nowrap = None;
        style.keep_spaces = None;
        style.preserve_newlines = None;
        style.break_after_spaces = None;
        return;
    }
    let value = value.to_ascii_lowercase();
    // CSS Text 3 section 3: an unknown keyword is not a normal declaration.
    if !matches!(
        value.as_str(),
        "normal" | "nowrap" | "pre" | "pre-wrap" | "pre-line" | "break-spaces"
    ) {
        return;
    }
    style.nowrap = Some(matches!(value.as_str(), "nowrap" | "pre"));
    style.keep_spaces = Some(matches!(
        value.as_str(),
        "pre" | "pre-wrap" | "break-spaces"
    ));
    style.preserve_newlines = Some(matches!(
        value.as_str(),
        "pre" | "pre-wrap" | "pre-line" | "break-spaces"
    ));
    style.break_after_spaces = Some(value == "break-spaces");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherit_replaces_all_components_and_invalid_keywords_preserve_them() {
        let mut style = Computed::default();
        apply(&mut style, "pre");
        apply(&mut style, "invalid");
        assert_eq!(style.nowrap, Some(true));
        assert_eq!(style.keep_spaces, Some(true));
        assert_eq!(style.preserve_newlines, Some(true));
        apply(&mut style, "inherit");
        assert_eq!(style.nowrap, None);
        assert_eq!(style.keep_spaces, None);
        assert_eq!(style.preserve_newlines, None);
        assert_eq!(style.break_after_spaces, None);
    }
}
