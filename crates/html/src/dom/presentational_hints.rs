//! HTML presentational attributes at the DOM/cascade boundary.
use crate::computed::Computed;
use crate::css::{Rule, Selector, parse_decls};

/// HTML rendering §15.3.8: cell nowrap is a presentational hint, with a
/// nonzero-length width exception in quirks mode. Author CSS takes precedence.
pub(super) fn nowrap(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    if !matches!(tag, "td" | "th") || !attrs.iter().any(|(k, _)| k == "nowrap") {
        return None;
    }
    let normal = super::quirks() && attrs.iter().any(|(k, v)| k == "width" && nonzero_length(v));
    Some(Rule {
        sel: Selector::parse("*")?,
        decls: parse_decls(if normal {
            "white-space: normal"
        } else {
            "white-space: nowrap"
        }),
        order: 0,
        origin: 1,
        // Below every author layer, above UA rules; part of author for revert.
        // CSS Cascade 5 §6.4: author presentational hint origin.
        layer: Vec::new(),
    })
}

/// HTML §2.3.4.4–5: a decimal numeric prefix, with % recognized immediately
/// after that prefix. Zero, a leading sign and a missing integer are invalid.
fn nonzero_length(raw: &str) -> bool {
    let bytes = raw
        .trim_start_matches(['\t', '\n', '\u{c}', '\r', ' '])
        .as_bytes();
    let mut i = 0;
    let mut nonzero = false;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        nonzero |= bytes[i] != b'0';
        i += 1;
    }
    if i == 0 {
        return false;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            nonzero |= bytes[i] != b'0';
            i += 1;
        }
    }
    nonzero && bytes.get(i) != Some(&b'%')
}

/// Preserve the existing bgcolor/text fallback below authored colors.
pub(super) fn colors(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    let color_of = |name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| crate::value::Color::parse(v.trim()))
    };
    if matches!(tag, "body" | "table" | "tr" | "td" | "th")
        && style.background.is_none()
        && style.gradient.is_none()
        && let Some(c) = color_of("bgcolor")
    {
        style.background = Some(c);
    }
    if tag == "body"
        && style.color.is_none()
        && let Some(c) = color_of("text")
    {
        style.color = Some(c);
    }
}
