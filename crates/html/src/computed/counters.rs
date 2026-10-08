//! Resolve calculated counter integers before applying counter scopes and scans.

use super::{Computed, split_outside_parens};

pub(super) fn apply(style: &mut Computed, key: &str, value: &str) {
    let Some(value) = normalized(value, key == "counter-reset") else {
        return;
    };
    let slot = match key {
        "counter-reset" => &mut style.counter_reset,
        "counter-increment" => &mut style.counter_increment,
        _ => &mut style.counter_set,
    };
    *slot = Some(value);
}

fn math(raw: &str) -> bool {
    ["calc(", "min(", "max(", "clamp("].iter().any(|prefix| {
        raw.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    })
}

fn name(raw: &str, reversed: bool) -> Option<String> {
    let reverse = raw
        .get(..9)
        .is_some_and(|head| head.eq_ignore_ascii_case("reversed("));
    let body = if reverse {
        if !reversed {
            return None;
        }
        raw[9..].strip_suffix(')')?.trim()
    } else {
        raw
    };
    if !crate::css::selector_tokens::ident(body)
        || matches!(
            body.to_ascii_lowercase().as_str(),
            "none" | "default" | "inherit" | "initial" | "unset" | "revert" | "revert-layer"
        )
    {
        return None;
    }
    Some(if reverse {
        format!("reversed({body})")
    } else {
        body.to_string()
    })
}

fn integer(raw: &str) -> Option<i32> {
    let digits = raw.strip_prefix(['+', '-']).unwrap_or(raw);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(
        raw.parse::<i128>()
            .map(|n| n.clamp(i32::MIN as i128, i32::MAX as i128) as i32)
            .unwrap_or(if raw.starts_with('-') {
                i32::MIN
            } else {
                i32::MAX
            }),
    )
}

// CSS Values 4 #calc-syntax requires whitespace around binary + and -.
// Keep it until the expression has been evaluated; counter scope tokenization
// subsequently removes spaces within parentheses for reversed() names.
fn operators_valid(raw: &str) -> bool {
    let ws = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}');
    for (at, ch) in raw.char_indices().filter(|(_, c)| matches!(c, '+' | '-')) {
        let previous = raw[..at].chars().rfind(|c| !ws(*c));
        if previous.is_none_or(|c| matches!(c, '(' | ',' | '*' | '/' | '+' | '-' | 'e' | 'E')) {
            continue;
        }
        if !raw[..at].chars().next_back().is_some_and(ws)
            || !raw[at + ch.len_utf8()..].chars().next().is_some_and(ws)
        {
            return false;
        }
    }
    true
}

fn normalized(raw: &str, reversed: bool) -> Option<String> {
    let words = split_outside_parens(raw);
    // Preserve existing non-mathematical directive parsing in this bounded fix.
    if !words.iter().any(|word| math(word)) {
        return Some(raw.to_string());
    }
    let mut out = Vec::new();
    let mut words = words.iter().peekable();
    while let Some(word) = words.next() {
        out.push(name(word, reversed)?);
        if let Some(next) = words.peek() {
            if math(next) {
                if !operators_valid(next) {
                    return None;
                }
                // CSS Values 4 §§5.1, 10.9: numeric calculations may occupy an
                // integer slot, with exact half ties rounded toward +infinity.
                let value = crate::value::number(&format!("calc({next})"))?;
                // CSS Values 4 §10.9.2 censors top-level NaN to zero.
                let value = if value.is_nan() { 0.0 } else { value };
                out.push(((value + 0.5).floor() as i32).to_string());
                words.next();
            } else if let Some(value) = integer(next) {
                out.push(value.to_string());
                words.next();
            }
        }
    }
    Some(out.join(" "))
}
