//! Разбор CSS: поиск `!` приоритета объявления на верхнем уровне значения —
//! вне строк, `url()`, скобок и экранов; `<!--` (CDO) — один токен
//! (CSS Syntax 3 §4.3.1), его восклицательный знак приоритетом не считается.

use super::{at_url, skip_string, skip_url};

pub(super) fn top_level_bang(value: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = 0usize;
    while at < value.len() {
        // CSS Syntax 3 §4.3.1: the exclamation belongs to one CDO token.
        if value[at..].starts_with("<!--") {
            at += 4;
            continue;
        }
        let ch = value[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += value[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&value[at..], ch);
                continue;
            }
            _ if at_url(&value[at..]) => {
                at += skip_url(&value[at..]);
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            '!' if depth == 0 => return Some(at),
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::parse_decls;

    #[test]
    fn cdo_values_participate_in_the_cascade() {
        let d = parse_decls("--a: red; --a: var(--b) <!--; color: var(--a) <!--");
        assert_eq!(d.last("--a"), Some("var(--b) <!--"));
        assert!(d.contains("color"));
    }

    #[test]
    fn cdo_does_not_hide_a_real_priority_marker() {
        assert!(parse_decls("--a: <!-- !important").contains("--a"));
        let d = parse_decls("--a: green; --a: <!-- !other");
        assert_eq!(d.last("--a"), Some("green"));
    }

    #[test]
    fn escaping_less_than_does_not_create_cdo() {
        let d = parse_decls(r"--a: green; --a: \<!--");
        assert_eq!(d.last("--a"), Some("green"));
    }
}
