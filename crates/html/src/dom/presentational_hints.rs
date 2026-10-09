//! HTML presentational attributes at the DOM/cascade boundary.
use crate::style::computed::Computed;
use crate::style::css::{Rule, Selector, parse_decls};

/// HTML rendering §15.3.8: cell nowrap is a presentational hint, with a
/// nonzero-length width exception in quirks mode. Author CSS takes precedence.
pub(super) fn nowrap(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    if !matches!(tag, "td" | "th") || !attrs.iter().any(|(k, _)| k == "nowrap") {
        return None;
    }
    let normal = crate::style::select::quirks() && attrs.iter().any(|(k, v)| k == "width" && nonzero_length(v));
    hint(if normal {
        "white-space: normal"
    } else {
        "white-space: nowrap"
    })
}

fn hint(css: &str) -> Option<Rule> {
    Some(Rule {
        sel: Selector::parse("*")?,
        decls: parse_decls(css),
        order: 0,
        origin: 1,
        // Below every author layer, above UA rules; part of author for revert.
        // CSS Cascade 5 §6.4: author presentational hint origin.
        layer: Vec::new(),
    })
}

/// HTML §15.3.8 maps table height to the author presentational-hint origin.
/// Keep it in the cascade so auto, initial, inherit and authored lengths win.
pub(super) fn rules(tag: &str, attrs: &[(String, String)]) -> Vec<Rule> {
    nowrap(tag, attrs)
        .into_iter()
        .chain(table_height(tag, attrs))
        .collect()
}

fn table_height(tag: &str, attrs: &[(String, String)]) -> Option<Rule> {
    if tag != "table" {
        return None;
    }
    let raw = &attrs.iter().find(|(name, _)| name == "height")?.1;
    let (value, percent) = dimension(raw)?;
    hint(&format!(
        "height: {value}{}",
        if percent { "%" } else { "px" }
    ))
}

/// HTML §2.3.4.4 accepts a nonnegative decimal prefix, including zero.
/// A percent sign must immediately follow that prefix; other suffixes are ignored.
fn dimension(raw: &str) -> Option<(f32, bool)> {
    let raw = raw.trim_start_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let bytes = raw.as_bytes();
    let mut end = 0;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    let value: f32 = raw[..end].parse().ok()?;
    value
        .is_finite()
        .then_some((value, bytes.get(end) == Some(&b'%')))
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
            .and_then(|(_, v)| crate::style::values::value::Color::parse(v.trim()))
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
