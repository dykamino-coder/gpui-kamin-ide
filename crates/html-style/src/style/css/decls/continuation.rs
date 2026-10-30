//! Переносы-продолжения внутри CSS-строк (CSS Syntax 3 §4.3.5).

/// Строки без переносов-продолжений: внутри строки `\` перед переводом
/// строки съедается вместе с ним (CSS Syntax 3 §4.3.5 «consume a string
/// token»: «if the next input code point is a newline, consume it»), так
/// многострочный `path("M 0 0 \` + перевод + `L 1 1")` даёт одну строку.
/// Прочие экранирования остаются как есть.
pub(in crate::style::css) fn join_string_continuations(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains('\\') {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len());
    let mut quote: Option<char> = None;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(_), '\\') => match chars.peek().copied() {
                Some('\n' | '\u{c}') => {
                    chars.next();
                }
                Some('\r') => {
                    chars.next();
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                }
                Some(next) => {
                    chars.next();
                    out.push(ch);
                    out.push(next);
                }
                None => out.push(ch),
            },
            (None, '\\') => {
                out.push(ch);
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            (Some(q), c) if c == q => {
                quote = None;
                out.push(c);
            }
            (None, '"' | '\'') => {
                quote = Some(ch);
                out.push(ch);
            }
            (_, c) => out.push(c),
        }
    }
    std::borrow::Cow::Owned(out)
}

#[cfg(test)]
mod continuation_tests {
    #[test]
    fn newline_after_backslash_inside_string_is_dropped() {
        let v = "path(\"M 0 0 \\\n L 1 1\") \\\n";
        assert_eq!(
            super::join_string_continuations(v),
            "path(\"M 0 0  L 1 1\") \\\n"
        );
        assert_eq!(super::join_string_continuations("'a\\'b'"), "'a\\'b'");
    }
}
