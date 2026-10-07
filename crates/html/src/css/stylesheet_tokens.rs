//! Distinguish stylesheet-level HTML comment delimiters from rule preludes.

/// CSS Syntax §consume-stylesheet-contents discards CDO/CDC between top-level statements,
/// while they remain ordinary input inside a rule or nested rule list.
pub(super) fn start(mut raw: &str) -> &str {
    loop {
        raw = raw.trim_start_matches([' ', '\t', '\n', '\r', '\u{c}']);
        match raw.strip_prefix("<!--").or_else(|| raw.strip_prefix("-->")) {
            Some(tail) => raw = tail,
            None => return raw,
        }
    }
}

pub(super) fn statement_head(raw: &str) -> bool {
    let Some(tail) = raw.trim_start().strip_prefix('@') else {
        return false;
    };
    // CSS Syntax §4.3.1 and §4.3.9: only an at-keyword ends at a semicolon.
    // A bare @ remains in a qualified rule's prelude until its first block.
    let mut chars = tail.chars();
    let first = chars.next();
    let second = chars.next();
    let third = chars.next();
    let name_start = |ch: Option<char>| {
        ch.is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_' || ch >= '\u{80}')
    };
    let escape = |a, b| a == Some('\\') && !matches!(b, Some('\n' | '\r' | '\u{c}'));
    match first {
        Some('-') => name_start(second) || second == Some('-') || escape(second, third),
        Some('\\') => escape(first, second),
        _ => name_start(first),
    }
}

#[cfg(test)]
mod tests {
    use super::statement_head;

    #[test]
    fn semicolons_end_only_at_keyword_rules() {
        for head in ["@import", " @-vendor", "@--custom", "@é", "@\\69mport"] {
            assert!(statement_head(head), "{head}");
        }
        for head in [
            "@ import",
            "@1import",
            "@-1import",
            "@\\\nimport",
            "",
            "div",
        ] {
            assert!(!statement_head(head), "{head}");
        }
    }
}
