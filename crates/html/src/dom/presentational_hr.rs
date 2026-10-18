//! HTML rendering §15.3.11 (the hr element): presentational hints of `<hr>`.
//!
//! The hints sit in the author presentational-hint origin like the table
//! hints next door (Servo `presentational-hints.css`: `hr[color],
//! hr[noshade] { border-style: solid }`, `hr[align=…]` margins; the
//! `size`/`width`/`color` mappings follow the spec text).

use crate::document::state::ValueCx;
use crate::style::css::Rule;

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// HTML §2.3.4.2 rules for parsing non-negative integers: the §2.3.4.1 integer
/// (optional `+`/`-`, digits, trailing junk ignored), an error when below zero
/// (`-0` is zero, `table-border-2`).
pub(super) fn non_negative_integer(raw: &str) -> Option<u32> {
    let raw = raw.trim_start_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let (negative, raw) = match raw.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, raw.strip_prefix('+').unwrap_or(raw)),
    };
    let digits: &str = &raw[..raw.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return None;
    }
    let value = digits
        .parse::<u64>()
        .map_or(u32::MAX, |v| v.min(u32::MAX as u64) as u32);
    (!negative || value == 0).then_some(value)
}

/// HTML §2.3.6 legacy colour value: the empty string and `transparent` fail;
/// CSS colour names and `#rrggbb` pass, bare hex digits get the missing `#`.
pub(super) fn legacy_color(raw: &str, vcx: ValueCx<'_>) -> Option<String> {
    let v = raw.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    if v.is_empty() || v.eq_ignore_ascii_case("transparent") {
        return None;
    }
    if crate::style::values::value::Color::parse(v, vcx).is_some() {
        return Some(v.to_string());
    }
    let hex = v.len() == 6 || v.len() == 3;
    (hex && v.bytes().all(|b| b.is_ascii_hexdigit())).then(|| format!("#{v}"))
}

/// The hint declarations of one `<hr>`.
pub(super) fn hints(attrs: &[(String, String)], vcx: ValueCx<'_>) -> String {
    let mut css = String::new();
    let color = attr(attrs, "color");
    let noshade = attr(attrs, "noshade").is_some();
    if let Some(c) = color.and_then(|c| legacy_color(c, vcx)) {
        css.push_str(&format!("color: {c};"));
    }
    // UA sheet: `hr[color], hr[noshade] { border-style: solid }` — presence only.
    if color.is_some() || noshade {
        css.push_str("border-style: solid;");
    }
    if let Some(n) = attr(attrs, "size").and_then(non_negative_integer) {
        if color.is_some() || noshade {
            css.push_str(&format!("border-width: {}px;", n as f32 / 2.0));
        } else if n == 1 {
            css.push_str("border-bottom-width: 0;");
        } else if n > 1 {
            css.push_str(&format!("height: {}px;", n - 2));
        }
    }
    if let Some((v, percent)) =
        attr(attrs, "width").and_then(super::presentational_hints::dimension)
    {
        css.push_str(&format!("width: {v}{};", if percent { "%" } else { "px" }));
    }
    match attr(attrs, "align").map(str::to_ascii_lowercase).as_deref() {
        Some("left") => css.push_str("margin-left: 0; margin-right: auto;"),
        Some("right") => css.push_str("margin-left: auto; margin-right: 0;"),
        Some("center") => css.push_str("margin-left: auto; margin-right: auto;"),
        _ => {}
    }
    css
}

pub(super) fn rule(tag: &str, attrs: &[(String, String)], vcx: ValueCx<'_>) -> Option<Rule> {
    if tag != "hr" {
        return None;
    }
    let css = hints(attrs, vcx);
    if css.is_empty() {
        return None;
    }
    super::presentational_hints::hint(&css)
}
