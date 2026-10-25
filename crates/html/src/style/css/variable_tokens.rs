//! Разбор CSS: синтаксическая проверка `var()` до попадания объявления в каскад
//! (CSS Variables §2.1, §3: негодный `var()` делает объявление недействительным,
//! даже если переменная есть и запас не понадобился бы). Здесь же общие
//! сканеры токенов — конец экрана, строки и блока (CSS Syntax §5.4.8).

pub(super) use crate::css_syntax::escape_end;

/// CSS Variables §2.1 and §3: invalid var() syntax invalidates the declaration,
/// even when the variable exists and its fallback would never be substituted.
pub(super) fn valid(value: &str) -> bool {
    sequence(value, false, 0)
}

fn sequence(raw: &str, fallback: bool, depth: usize) -> bool {
    if depth > 128 {
        return false;
    }
    let mut at = 0;
    while at < raw.len() {
        // CDO is a token, not the top-level ! delimiter forbidden in a fallback.
        if raw[at..].starts_with("<!--") {
            at += 4;
            continue;
        }
        let c = raw[at..].chars().next().unwrap();
        if raw[at..].starts_with("/*") {
            at += raw[at + 2..].find("*/").map_or(raw.len() - at, |n| n + 4);
            continue;
        }
        if matches!(c, '\'' | '"') {
            let Some(end) = string_end(raw, at, c) else {
                return false;
            };
            at = end;
            continue;
        }
        if matches!(c, ')' | ']' | '}') || fallback && matches!(c, '!' | ';') {
            return false;
        }
        let start = at;
        if c == '\\' || c == '-' || c == '_' || c.is_alphanumeric() || c >= '\u{80}' {
            while at < raw.len() {
                let ch = raw[at..].chars().next().unwrap();
                if ch == '\\' {
                    at = escape_end(raw, at);
                } else if ch.is_alphanumeric() || matches!(ch, '-' | '_') || ch >= '\u{80}' {
                    at += ch.len_utf8();
                } else {
                    break;
                }
            }
        }
        let is_var = at > start && super::unescape(&raw[start..at]).eq_ignore_ascii_case("var");
        let ch = raw[at..].chars().next();
        if let Some(open @ ('(' | '[' | '{')) = ch {
            let Some(end) = block_end(raw, at, open) else {
                return false;
            };
            let inner = &raw[at + 1..end];
            if is_var && open == '(' {
                let clean = super::strip_comments(inner);
                let args = super::split_top_level(&clean, ',');
                // css-variables-2 §3: the argument grammar is
                // `var( <declaration-value> , <declaration-value>? )`; the
                // name is checked against <custom-property-name> only at
                // substitution time (`var(1px)`, `var(--)` parse fine).
                if args[0].trim().is_empty() || !sequence(inner, true, depth + 1) {
                    return false;
                }
            } else if !sequence(inner, false, depth + 1) {
                return false;
            }
            at = (end + 1).min(raw.len());
        } else if at == start {
            at += c.len_utf8();
        }
    }
    true
}

pub(super) fn string_end(raw: &str, start: usize, quote: char) -> Option<usize> {
    let body = start + 1;
    crate::css_syntax::scan_string(&raw[body..], quote)
        .ok()
        .map(|end| body + end)
}

pub(crate) fn block_end(raw: &str, start: usize, open: char) -> Option<usize> {
    let close = match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    };
    let mut stack = vec![close];
    let mut at = start + 1;
    while at < raw.len() {
        let c = raw[at..].chars().next()?;
        if raw[at..].starts_with("/*") {
            at += raw[at + 2..].find("*/").map_or(raw.len() - at, |n| n + 4);
            continue;
        }
        if matches!(c, '\'' | '"') {
            at = string_end(raw, at, c)?;
            continue;
        }
        if c == '\\' {
            at = escape_end(raw, at);
            continue;
        }
        match c {
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' => {
                if stack.pop()? != c {
                    return None;
                }
                if stack.is_empty() {
                    return Some(at);
                }
            }
            _ => {}
        }
        at += c.len_utf8();
    }
    // CSS Syntax §5.4.8 closes outstanding blocks at end of input.
    Some(raw.len())
}

#[cfg(test)]
mod tests {
    use super::super::parse_decls;

    #[test]
    fn closing_quote_before_newline_is_not_a_bad_string() {
        assert!(super::valid("var(--a, \"green\"\n)"));
        assert!(!super::valid("var(--a, \"green\n)"));
        assert!(!super::valid("var(--a, \"green\\\"\n)"));
        assert!(super::valid("var(--a, \"green"));
    }

    #[test]
    fn invalid_fallback_does_not_replace_a_previous_declaration() {
        for fallback in ["!", "!important", ";", "var(,green)"] {
            let decls = parse_decls(&format!("--a: green; --a: var(--b,{fallback})"));
            assert_eq!(decls.last("--a"), Some("green"));
        }
    }

    #[test]
    fn fallback_tokens_inside_blocks_and_strings_remain_valid() {
        for value in [
            "var(--a,)",
            "var(--a, <!--)",
            "var(--a, rgb(0, 0, 0))",
            "var(--a, [!;])",
            "var(--a, \"!;var(1px)\")",
            "var(--a, var(--b, green))",
            "var(/* before */--a/* , */, green)",
            "1var(--a,!)",
            "var(1px)",
            "var(--, green)",
        ] {
            assert!(super::valid(value), "{value}");
        }
        assert!(!super::valid("var(--a, var(--b, !))"));
        assert!(!super::valid("var(--a, <! --)"));
        assert!(!super::valid("red)"));
    }

    #[test]
    fn custom_names_preserve_case_and_empty_values() {
        let decls = parse_decls("--a: green; --A: red; --empty:; --: red; COLOR: blue");
        assert_eq!(decls.last("--a"), Some("green"));
        assert_eq!(decls.last("--A"), Some("red"));
        assert_eq!(decls.last("--empty"), Some(""));
        assert!(!decls.contains("--"));
        assert!(decls.contains("color"));
    }
}
