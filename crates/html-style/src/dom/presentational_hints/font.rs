//! Legacy font face, size and color hints (HTML Rendering, Phrasing content).
//! All three properties inherit through the ordinary cascade.

use crate::style::css::{LonghandId, Rule};
use crate::values::ValueCx;

pub(super) fn rule(tag: &str, attrs: &[(String, String)], vcx: ValueCx<'_>) -> Option<Rule> {
    if tag != "font" {
        return None;
    }
    let attr = |name| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    let mut rule = super::hint("")?;
    if let Some(face) = attr("face")
        && LonghandId::FontFamily.accepts(face, vcx)
    {
        // A single declaration prevents an attribute from injecting another
        // property or an importance flag.
        rule.decls.push("font-family", face, false);
    }
    if let Some(size) = attr("size").and_then(legacy_size) {
        rule.decls.push("font-size", size, false);
    }
    if let Some(color) =
        attr("color").and_then(|raw| super::super::presentational_hr::legacy_color(raw, vcx))
    {
        rule.decls.push("color", color, false);
    }
    (!rule.decls.is_empty()).then_some(rule)
}

/// HTML rules for parsing a legacy font size: a sign selects an offset from
/// 3; the integer prefix saturates to 1..=7 independently of the parent.
fn legacy_size(raw: &str) -> Option<&'static str> {
    let raw = raw.trim_start_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let (digits, sign) = match raw.as_bytes().first() {
        Some(b'+') => (&raw[1..], 1),
        Some(b'-') => (&raw[1..], -1),
        _ => (raw, 0),
    };
    let mut number = 0i32;
    let mut found = false;
    for digit in digits.bytes().take_while(u8::is_ascii_digit) {
        found = true;
        number = (number * 10 + i32::from(digit - b'0')).min(7);
    }
    if !found {
        return None;
    }
    let number = if sign == 0 { number } else { 3 + sign * number };
    // The legacy table starts at x-small, then follows CSS Fonts 4 keywords.
    Some(
        [
            "x-small",
            "small",
            "medium",
            "large",
            "x-large",
            "xx-large",
            "xxx-large",
        ][(number.clamp(1, 7) - 1) as usize],
    )
}
