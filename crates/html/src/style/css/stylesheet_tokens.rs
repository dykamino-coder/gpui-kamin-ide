//! Consume stylesheet preludes and nested blocks without leaking their tokens.

use super::{Piece, at_url, skip_string, skip_url};

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

/// CSS Syntax sections 5.4.2-5.4.3 consume nested component values as units.
/// A mismatched closing token never closes a block of another delimiter type.
pub(crate) fn next_piece(text: &str) -> Option<(Piece<'_>, &str)> {
    let mut stack = Vec::new();
    let mut at = 0usize;
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&text[at..], ch);
                continue;
            }
            _ if at_url(&text[at..]) => {
                at += skip_url(&text[at..]);
                continue;
            }
            // Точка с запятой кончает только AT-правило-предложение. У
            // обычного правила она — часть преамбулы до `{` (css-syntax-3
            // §5.4.3 «consume a qualified rule»): `test; @charset "x";
            // .a, #b { color: red }` — ОДНО правило с негодным селектором, и
            // отбрасывается оно целиком (`at-charset-039`). Прежде `test;`
            // обрывалось на месте, и красное правило оживало.
            ';' if stack.is_empty() && statement_head(text) => {
                let head = &text[..at];
                return Some((Piece::Statement { head }, &text[at + 1..]));
            }
            '{' if stack.is_empty() => {
                let head = &text[..at];
                let rest = &text[at..];
                let (body, tail) = match find_matching(rest) {
                    Some(close) => (&rest[1..close], &rest[close + 1..]),
                    // Незакрытый блок в КОНЦЕ таблицы закрывается неявно
                    // (§5.4.1): правило всё равно действует.
                    None => (&rest[1..], ""),
                };
                return Some((Piece::Block { head, body }, tail));
            }
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' if stack.last() == Some(&ch) => {
                stack.pop();
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

/// CSS Syntax section 5.4.8: each simple block ends at its matching token.
pub(super) fn find_matching(from_brace: &str) -> Option<usize> {
    let mut stack = Vec::new();
    let bytes = from_brace.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        let ch = from_brace[at..].chars().next().unwrap_or('\0');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += from_brace[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&from_brace[at..], ch);
                continue;
            }
            _ if at_url(&from_brace[at..]) => {
                at += skip_url(&from_brace[at..]);
                continue;
            }
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' if stack.last() == Some(&ch) => {
                stack.pop();
                if stack.is_empty() {
                    return Some(at);
                }
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
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
