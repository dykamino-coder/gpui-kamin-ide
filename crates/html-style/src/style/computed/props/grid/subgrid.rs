//! Грамматика подсетки css-grid-2 §subgrid-listing: только имена линий,
//! без размеров дорожек; всё объявление проверяется до записи полей стиля.

use crate::style::css::{block_end, selector_tokens, split_top_level};
use crate::style::values::value::number;
use crate::values::ValueCx;

pub(super) fn valid(v: &str, vcx: ValueCx<'_>) -> bool {
    parse(v, vcx).is_some()
}

pub(super) fn parse(v: &str, vcx: ValueCx<'_>) -> Option<gpui::GridAxisLineNames> {
    let v = v.trim_matches(space);
    if !v.get(..7)?.eq_ignore_ascii_case("subgrid") {
        return None;
    }
    let mut rest = &v[7..];
    if !rest.is_empty() && !rest.starts_with(['[', ' ', '\t', '\n', '\r', '\u{c}']) {
        return None;
    }
    let mut out = gpui::GridAxisLineNames::default();
    let mut lines = Vec::new();
    loop {
        rest = rest.trim_start_matches(space);
        if rest.is_empty() {
            break;
        }
        if rest.starts_with('[') {
            let (names, tail) = names(rest)?;
            lines.push(names);
            rest = tail;
            continue;
        }
        let open = rest.find('(')?;
        if !rest[..open].eq_ignore_ascii_case("repeat") {
            return None;
        }
        let end = block_end(rest, open, '(')?;
        let args = split_top_level(rest.get(open + 1..end)?, ',');
        let [count, body] = args.as_slice() else {
            return None;
        };
        let mut body = body.trim_matches(space);
        let mut repeated = Vec::new();
        while !body.is_empty() {
            let (names, tail) = names(body)?;
            repeated.push(names);
            body = tail.trim_start_matches(space);
        }
        if repeated.is_empty() {
            return None;
        }
        let count = count.trim_matches(space);
        if count.eq_ignore_ascii_case("auto-fill") {
            if out.repeat.is_some() {
                return None;
            }
            out.before = std::mem::take(&mut lines);
            out.repeat = Some(repeated);
        } else {
            let n = repeat_count(count, vcx)?;
            // Тот же предел раскрытия счётного repeat, что у списка дорожек.
            for _ in 0..n.min(64) {
                lines.extend(repeated.iter().cloned());
            }
        }
        rest = rest.get(end + 1..)?;
    }
    if out.repeat.is_some() {
        out.after = lines;
    } else {
        out.before = lines;
    }
    Some(out)
}

fn repeat_count(v: &str, vcx: ValueCx<'_>) -> Option<usize> {
    let digits = v.strip_prefix('+').unwrap_or(v);
    if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
        return digits.bytes().any(|c| c != b'0').then(|| {
            digits.bytes().fold(0usize, |n, c| {
                n.saturating_mul(10).saturating_add((c - b'0') as usize)
            })
        });
    }
    // CSS Values 4 §10: числовая математика заменяет integer, округляется
    // к ближайшему целому (половина вверх) и ограничивается диапазоном типа.
    let open = v.find('(')?;
    if !selector_tokens::ident(&v[..open])
        || block_end(v, open, '(')? != v.len() - 1
        || v.chars().any(|c| c.is_whitespace() && !space(c))
    {
        return None;
    }
    let n = number(&format!("calc({})", v.to_ascii_lowercase()), vcx)?;
    Some((n + 0.5).floor().max(1.0) as usize)
}

fn space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

fn names(v: &str) -> Option<(Vec<String>, &str)> {
    if !v.starts_with('[') {
        return None;
    }
    let end = block_end(v, 0, '[')?;
    let names = selector_tokens::ident_list(v.get(1..end)?)?;
    if names.iter().any(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "span"
                | "auto"
                | "initial"
                | "inherit"
                | "unset"
                | "revert"
                | "revert-layer"
                | "revert-rule"
                | "default"
        )
    }) {
        return None;
    }
    Some((names, v.get(end + 1..)?))
}

#[cfg(test)]
mod tests;
