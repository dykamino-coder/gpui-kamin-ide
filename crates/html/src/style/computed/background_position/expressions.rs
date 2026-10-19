//! Retain comparison syntax and substitute percentages only at used-value time.

use crate::document::state::ValueCx;
use crate::style::computed::resolve::image_lengths::lengths;
use crate::style::computed::split_outside_parens;
use crate::style::values::value::Len;

pub(super) fn math(raw: &str, vcx: ValueCx<'_>) -> Option<String> {
    let low = raw.to_ascii_lowercase();
    if !low.contains('%')
        || !["min(", "max(", "clamp("]
            .iter()
            .any(|name| low.contains(name))
    {
        return None;
    }
    let typed = lengths(raw, 16.0, 16.0, 8.0, 8.0, 16.0);
    resolve_position_math(&typed, 1.0, vcx).map(|_| raw.to_string())
}

pub(super) fn capture(raw: &str, vcx: ValueCx<'_>) -> [Option<String>; 2] {
    let first = crate::style::css::split_args(raw)
        .into_iter()
        .next()
        .unwrap_or("");
    let words = split_outside_parens(first);
    let keyword = |s: &str| matches!(s, "left" | "right" | "top" | "bottom" | "center");
    let mut result = [None, None];
    if words.len() >= 3 {
        let mut i = 0;
        while i < words.len() {
            let side = words[i].as_str();
            if !keyword(side) {
                return [None, None];
            }
            i += 1;
            let offset = words.get(i).filter(|s| !keyword(s));
            if let Some(offset) = offset {
                if Len::parse_mixed(offset, vcx).is_none() && math(offset, vcx).is_none() {
                    return [None, None];
                }
                i += 1;
                if let Some(expression) = math(offset, vcx)
                    && side != "center"
                {
                    let axis = usize::from(matches!(side, "top" | "bottom"));
                    result[axis] = Some(if matches!(side, "right" | "bottom") {
                        format!("calc(100% - ({expression}))")
                    } else {
                        expression
                    });
                }
            }
        }
        return result;
    }
    let fixed = [
        words.iter().any(|w| matches!(w.as_str(), "left" | "right")),
        words.iter().any(|w| matches!(w.as_str(), "top" | "bottom")),
    ];
    let mut values = words
        .iter()
        .filter(|w| !matches!(w.as_str(), "left" | "right" | "top" | "bottom"));
    for axis in 0..2 {
        if !fixed[axis] {
            result[axis] = values.next().and_then(|v| math(v, vcx));
        }
    }
    result
}

/// Resolve every percentage to a length BEFORE evaluating min/max/clamp.
/// This keeps the existing arithmetic parser and dimensional type checks,
/// without changing comparison simplification for other CSS properties.
pub(super) fn resolve_position_math(raw: &str, basis: f32, vcx: ValueCx<'_>) -> Option<f32> {
    let bytes = raw.as_bytes();
    let mut normalized = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() || bytes[i] == b'.' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i < bytes.len() && matches!(bytes[i], b'e' | b'E') {
                let exponent = i;
                i += 1;
                if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
                    i += 1;
                }
                let digits = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i == digits {
                    i = exponent;
                }
            }
            if bytes.get(i) == Some(&b'%') {
                let value = (raw[start..i].parse::<f32>().ok()? / 100.0) * basis;
                normalized.push_str(&format!("({value}px)"));
                i += 1;
            } else {
                normalized.push_str(&raw[start..i]);
            }
        } else {
            let ch = raw[i..].chars().next()?;
            normalized.push(ch);
            i += ch.len_utf8();
        }
    }
    match Len::parse(&normalized, vcx)? {
        Len::Px(value) if value.is_finite() => Some(value),
        _ => None,
    }
}
