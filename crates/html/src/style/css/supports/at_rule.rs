//! CSS Conditional 5 at-rule(): one at-keyword naming a supported rule in any context.

use super::*;

pub(super) fn supported(args: &str) -> bool {
    let Some(raw) = args.trim().strip_prefix('@') else {
        return false;
    };
    if !selector_tokens::ident(raw) {
        return false;
    }
    let name = unescape(raw).to_ascii_lowercase();
    // Keep this registry aligned with sheet/rules, keyframes, font-face and
    // font-feature-values. @charset is an encoding declaration, never an at-rule.
    matches!(
        name.as_str(),
        "supports"
            | "media"
            | "layer"
            | "scope"
            | "namespace"
            | "import"
            | "page"
            | "font-face"
            | "keyframes"
            | "property"
            | "counter-style"
            | "position-try"
            | "font-feature-values"
            | "stylistic"
            | "styleset"
            | "character-variant"
            | "swash"
            | "ornaments"
            | "annotation"
    ) || MARGIN_BOXES.contains(&name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_keyword_is_one_token_and_ascii_case_insensitive() {
        for condition in [
            "at-rule(@supports)",
            "AT-RULE(@SuPpOrTs)",
            r"at-rule(@support\73)",
            r"at\2d rule(@supports)",
            "at-rule(@top-left)",
            "at-rule(@styleset)",
        ] {
            assert!(supports(condition), "{condition}");
        }
        for args in [
            "",
            "supports",
            "@",
            "@ supports",
            "@supports extra",
            "@supports, @media",
            "@supports()",
            "'@supports'",
            "@1supports",
            "@charset",
            "@container",
            "@unknown",
        ] {
            assert!(!supported(args), "{args}");
            assert!(supports(&format!("not at-rule({args})")), "{args}");
        }
    }
}
