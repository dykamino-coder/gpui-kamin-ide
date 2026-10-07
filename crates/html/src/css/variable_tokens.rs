//! Validate substitution functions before declarations enter the cascade.

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
                let name = args[0].trim();
                if !super::selector_tokens::ident(name)
                    || !super::unescape(name).starts_with("--")
                    || super::unescape(name) == "--"
                    || !sequence(inner, true, depth + 1)
                {
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

fn escape_end(raw: &str, at: usize) -> usize {
    let mut end = at + 1;
    let mut digits = 0;
    while end < raw.len() && digits < 6 && raw.as_bytes()[end].is_ascii_hexdigit() {
        end += 1;
        digits += 1;
    }
    if let Some(c) = raw[end..].chars().next()
        && (digits == 0 || c.is_ascii_whitespace())
    {
        end += c.len_utf8();
        if c == '\r' && raw[end..].starts_with('\n') {
            end += 1;
        }
    }
    end
}

fn string_end(raw: &str, start: usize, quote: char) -> Option<usize> {
    let mut at = start + 1;
    while at < raw.len() {
        let c = raw[at..].chars().next()?;
        if c == quote {
            return Some(at + 1);
        }
        if matches!(c, '\n' | '\r' | '\u{c}') {
            return None;
        }
        at = if c == '\\' {
            escape_end(raw, at)
        } else {
            at + c.len_utf8()
        };
    }
    Some(raw.len())
}

fn block_end(raw: &str, start: usize, open: char) -> Option<usize> {
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
    fn invalid_fallback_does_not_replace_a_previous_declaration() {
        for fallback in ["!", "!important", ";", "var(1px)"] {
            let decls = parse_decls(&format!("--a: green; --a: var(--b,{fallback})"));
            assert_eq!(decls.get("--a").map(String::as_str), Some("green"));
        }
    }

    #[test]
    fn fallback_tokens_inside_blocks_and_strings_remain_valid() {
        for value in [
            "var(--a,)",
            "var(--a, rgb(0, 0, 0))",
            "var(--a, [!;])",
            "var(--a, \"!;var(1px)\")",
            "var(--a, var(--b, green))",
            "var(/* before */--a/* , */, green)",
            "1var(--a,!)",
        ] {
            assert!(super::valid(value), "{value}");
        }
        assert!(!super::valid("var(--a, var(--b, !))"));
        assert!(!super::valid("red)"));
    }

    #[test]
    fn custom_names_preserve_case_and_empty_values() {
        let decls = parse_decls("--a: green; --A: red; --empty:; --: red; COLOR: blue");
        assert_eq!(decls.get("--a").map(String::as_str), Some("green"));
        assert_eq!(decls.get("--A").map(String::as_str), Some("red"));
        assert_eq!(decls.get("--empty").map(String::as_str), Some(""));
        assert!(!decls.contains_key("--"));
        assert!(decls.contains_key("color"));
    }
}
