//! Validate selector identifier and string tokens before unescaping their contents.

use super::{AttrSel, unescape};

/// CSS Syntax §4.3.9: digits may continue an identifier, but cannot start it.
/// Escapes count as name-start characters regardless of the decoded character.
pub(crate) fn ident(raw: &str) -> bool {
    let mut chars = raw.chars().peekable();
    if chars.peek() == Some(&'-') {
        chars.next();
        if chars.peek() == Some(&'-') {
            chars.next();
        } else if !name_start(&mut chars) {
            return false;
        }
    } else if !name_start(&mut chars) {
        return false;
    }
    while let Some(c) = chars.next() {
        if c == '\\' {
            if !escape(&mut chars) {
                return false;
            }
        } else if !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c >= '\u{80}') {
            return false;
        }
    }
    true
}

fn name_start(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    match chars.next() {
        Some('\\') => escape(chars),
        Some(c) => c.is_ascii_alphabetic() || c == '_' || c >= '\u{80}',
        None => false,
    }
}

fn escape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let Some(c) = chars.next() else { return false };
    if matches!(c, '\n' | '\r' | '\u{c}') {
        return false;
    }
    if c.is_ascii_hexdigit() {
        for _ in 1..6 {
            if chars.peek().is_some_and(char::is_ascii_hexdigit) {
                chars.next();
            } else {
                break;
            }
        }
        if chars
            .peek()
            .is_some_and(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}'))
        {
            if chars.next() == Some('\r') && chars.peek() == Some(&'\n') {
                chars.next();
            }
        }
    }
    true
}

/// Attribute values must be one identifier or one complete CSS string token.
pub(super) fn value(raw: &str) -> Option<String> {
    let Some(quote @ ('"' | '\'')) = raw.chars().next() else {
        return ident(raw).then(|| unescape(raw));
    };
    // CSS Syntax §3.3 normalizes CRLF, CR and form feed before tokenization.
    let body = raw
        .strip_prefix(quote)?
        .strip_suffix(quote)?
        .replace("\r\n", "\n")
        .replace(['\r', '\u{c}'], "\n");
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            } else if !escape(&mut chars) {
                return None;
            }
        } else if c == quote || c == '\n' {
            return None;
        }
    }
    // CSS Syntax §4.3.5 consumes a backslash-newline without appending either.
    // Remove continuations before decoding: a hexadecimal \A remains a newline.
    Some(unescape(&body.replace("\\\n", "")))
}

pub(super) fn attr(raw: &str) -> Option<AttrSel> {
    let raw = raw.trim();
    let op_at = raw.char_indices().find(|(i, c)| {
        *c == '=' || matches!(c, '~' | '|' | '^' | '$' | '*') && raw[i + 1..].starts_with('=')
    });
    let Some((i, op_ch)) = op_at else {
        if !ident(raw) {
            return None;
        }
        return Some(AttrSel {
            name: unescape(raw).to_ascii_lowercase(),
            op: None,
            ci: false,
        });
    };
    let name = raw[..i].trim();
    if !ident(name) {
        return None;
    }
    let (op, val_start) = match op_ch {
        '=' => (0u8, i + 1),
        '~' => (1, i + 2),
        '|' => (2, i + 2),
        '^' => (3, i + 2),
        '$' => (4, i + 2),
        _ => (5, i + 2),
    };
    let mut value = raw[val_start..].trim();
    let mut ci = false;
    if let Some(stripped) = value
        .strip_suffix('i')
        .or_else(|| value.strip_suffix('I'))
        .map(str::trim_end)
        && (stripped.ends_with('"')
            || stripped.ends_with('\'')
            || stripped.ends_with(char::is_whitespace))
    {
        ci = true;
        value = stripped.trim_end();
    }
    let value = self::value(value)?;
    let name = unescape(name).to_ascii_lowercase();
    Some(AttrSel {
        ci: ci || CI_ATTRS.contains(&name.as_str()),
        name,
        op: Some((op, value)),
    })
}

/// Атрибуты HTML, значения которых сравниваются БЕЗ учёта регистра даже без
/// флага ` i` (HTML, «Case-sensitivity of selectors»). Перечень закрытый:
/// прочие атрибуты сравниваются посимвольно.
const CI_ATTRS: &[&str] = &[
    "accept",
    "accept-charset",
    "align",
    "alink",
    "axis",
    "bgcolor",
    "charset",
    "checked",
    "clear",
    "codetype",
    "color",
    "compact",
    "declare",
    "defer",
    "dir",
    "direction",
    "disabled",
    "enctype",
    "face",
    "frame",
    "hreflang",
    "http-equiv",
    "lang",
    "language",
    "link",
    "media",
    "method",
    "multiple",
    "nohref",
    "noresize",
    "noshade",
    "nowrap",
    "readonly",
    "rel",
    "rev",
    "rules",
    "scope",
    "scrolling",
    "selected",
    "shape",
    "target",
    "text",
    "type",
    "valign",
    "valuetype",
    "vlink",
];
