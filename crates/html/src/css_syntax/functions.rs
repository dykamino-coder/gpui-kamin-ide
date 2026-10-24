//! Имена функций CSS: строки, комментарии, URL и экраны непрозрачны.

use super::{escape_end, skip_string, skip_url_body, unescape};

pub(crate) struct Function<'a> {
    pub start: usize,
    pub open: usize,
    pub name: std::borrow::Cow<'a, str>,
}

pub(crate) fn functions(raw: &str) -> impl Iterator<Item = Function<'_>> {
    let mut at = 0;
    std::iter::from_fn(move || {
        while at < raw.len() {
            let ch = raw[at..].chars().next()?;
            if raw[at..].starts_with("/*") {
                at += raw[at + 2..].find("*/").map_or(raw.len() - at, |n| n + 4);
                continue;
            }
            if matches!(ch, '\'' | '"') {
                at += 1;
                at += skip_string(&raw[at..], ch);
                continue;
            }
            let start = at;
            let prefixed = matches!(ch, '#' | '@');
            if prefixed {
                at += 1;
            }
            while let Some(c) = raw[at..].chars().next() {
                if c == '\\' {
                    at = escape_end(raw, at);
                } else if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') || c >= '\u{80}' {
                    at += c.len_utf8();
                } else {
                    break;
                }
            }
            if at > start && !prefixed && raw[at..].starts_with('(') {
                let name = if raw[start..at].contains('\\') {
                    std::borrow::Cow::Owned(unescape(&raw[start..at]))
                } else {
                    std::borrow::Cow::Borrowed(&raw[start..at])
                };
                let open = at;
                at += 1;
                if name.eq_ignore_ascii_case("url") {
                    at = open + 1 + skip_url_body(&raw[open + 1..]);
                }
                return Some(Function { start, open, name });
            }
            if at == start {
                at += ch.len_utf8();
            }
        }
        None
    })
}
