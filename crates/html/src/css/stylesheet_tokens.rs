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
    let raw = raw.trim_start();
    raw.starts_with('@') || raw.is_empty()
}
