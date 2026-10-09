//! Preserve implicit EOF termination when component values are kept as source text.

use std::borrow::Cow;

/// CSS Syntax §4.3.5, §5.4.8 and §5.4.9 return strings, blocks and functions
/// at EOF. Our property grammars consume their serialized form, so retain the
/// implicit ending tokens rather than rejecting a valid final declaration.
pub(super) fn complete(raw: &str) -> Cow<'_, str> {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut content_end = raw.len();
    let mut at = 0;
    while at < raw.len() {
        if raw[at..].starts_with("/*") {
            let Some(end) = raw[at + 2..].find("*/") else {
                return Cow::Borrowed(raw);
            };
            at += end + 4;
            continue;
        }
        let ch = raw[at..].chars().next().unwrap();
        if matches!(ch, '\'' | '"') {
            let Some(end) = super::variable_tokens::string_end(raw, at, ch) else {
                // A newline is a bad-string token, not implicit EOF termination.
                return Cow::Borrowed(raw);
            };
            let closed =
                end > at + 1 && raw[..end].ends_with(ch) && backslashes(&raw[..end - 1]).is_multiple_of(2);
            if !closed {
                quote = Some(ch);
                // A terminal backslash in a string contributes no character.
                if !backslashes(raw).is_multiple_of(2) {
                    content_end -= 1;
                }
                break;
            }
            at = end;
            continue;
        }
        if ch == '\\' {
            at = super::variable_tokens::escape_end(raw, at);
            continue;
        }
        if super::at_url(&raw[at..]) && !raw[at + 4..].trim_start().starts_with(['\'', '"']) {
            let end = at + super::skip_url(&raw[at..]);
            if !raw[..end].ends_with(')') || !backslashes(&raw[..end - 1]).is_multiple_of(2) {
                stack.push(')');
            }
            at = end;
            continue;
        }
        match ch {
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}'
                if stack.pop() != Some(ch) => {
                    return Cow::Borrowed(raw);
                }
            _ => {}
        }
        at += ch.len_utf8();
    }
    if quote.is_none() && stack.is_empty() {
        return Cow::Borrowed(raw);
    }
    let mut result = raw[..content_end].to_string();
    if let Some(quote) = quote {
        result.push(quote);
    }
    result.extend(stack.into_iter().rev());
    Cow::Owned(result)
}

fn backslashes(raw: &str) -> usize {
    raw.bytes().rev().take_while(|ch| *ch == b'\\').count()
}

/// Byte length of a string body, including its closing quote if present.
/// Escapes hide delimiters; a bad string ends before its unescaped newline.
pub(crate) fn skip_string(text: &str, quote: char) -> usize {
    let mut at = 0;
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap();
        if ch == '\\' {
            // A hexadecimal escape consumes its terminating CSS whitespace,
            // including a newline; that newline cannot end a bad-string token.
            at = super::variable_tokens::escape_end(text, at);
            continue;
        }
        if ch == quote {
            return at + ch.len_utf8();
        }
        if matches!(ch, '\n' | '\r' | '\u{c}') {
            return at;
        }
        at += ch.len_utf8();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::{complete, skip_string};

    #[test]
    fn escape_terminators_are_consumed_before_bad_string_detection() {
        for text in [
            "a\\00000a\nb\"",
            "a\\a\r\nb\"",
            "a\\\r\nb\"",
            "a\\c\u{c}b\"",
        ] {
            assert_eq!(skip_string(text, '"'), text.len());
        }
        for text in ["a\nb\"", "a\rb\"", "a\u{c}b\""] {
            assert_eq!(skip_string(text, '"'), 1);
        }
    }

    #[test]
    fn unfinished_component_values_keep_their_implicit_ends() {
        for (raw, expected) in [
            ("color: rgb(0, 128, 0", "color: rgb(0, 128, 0)"),
            ("content: \"text", "content: \"text\""),
            ("content: \"text\\", "content: \"text\""),
            (
                "content: attr(data-x, \"text",
                "content: attr(data-x, \"text\")",
            ),
            ("background: url(foo[", "background: url(foo[)"),
        ] {
            assert_eq!(complete(raw), expected);
        }
    }

    #[test]
    fn bad_strings_and_unmatched_closers_stay_invalid() {
        for raw in ["content: \"text\n", "color: rgb(0, 128, 0]", "color: red"] {
            assert_eq!(complete(raw), raw);
        }
    }
}
