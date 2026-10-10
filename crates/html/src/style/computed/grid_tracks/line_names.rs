//! Имена линий сетки и размещение по ним: `[a b]`, repeat() с именами, grid-row/column по имени и span.

use super::*;

/// Токены записи шаблона для имён линий: `[имена]`, `функция(…)`, слова.
pub(super) fn line_name_tokens(v: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let (mut paren, mut bracket) = (0i32, 0i32);
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let t = cur.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
        cur.clear();
    };
    for ch in v.chars() {
        match ch {
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren -= 1;
                cur.push(ch);
            }
            '[' if paren == 0 => {
                if bracket == 0 {
                    flush(&mut cur, &mut out);
                }
                bracket += 1;
                cur.push(ch);
            }
            ']' if paren == 0 => {
                bracket -= 1;
                cur.push(ch);
                if bracket == 0 {
                    flush(&mut cur, &mut out);
                }
            }
            c if c.is_whitespace() && paren == 0 && bracket == 0 => flush(&mut cur, &mut out),
            _ => cur.push(ch),
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// `[a b]` → `["a", "b"]`.
fn bracket_names(t: &str) -> Option<Vec<String>> {
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    Some(inner.split_whitespace().map(str::to_string).collect())
}

/// Имена линий тела `repeat(…)`: у шаблона — по линиям вокруг дорожек тела
/// (дорожек + 1), у `<line-name-list>` подсетки — по записи на линию.
fn repeat_body_names(rest: &str, subgrid: bool) -> Vec<Vec<String>> {
    let mut lines: Vec<Vec<String>> = if subgrid {
        Vec::new()
    } else {
        vec![Vec::new()]
    };
    for t in line_name_tokens(rest) {
        match bracket_names(&t) {
            Some(names) if subgrid => lines.push(names),
            Some(names) => {
                if let Some(last) = lines.last_mut() {
                    last.extend(names);
                }
            }
            None if !subgrid => lines.push(Vec::new()),
            None => {}
        }
    }
    lines
}

/// Имена линий записи `grid-template-*` (css-grid-2 §7.2.2 `<line-names>`,
/// §subgrid-listing `<line-name-list>`): по списку имён на линию между
/// КОМПОНЕНТАМИ шаблона — `repeat(N, …)` раскрыт на месте, как в
/// `parse_tracks`, а `repeat(auto-fill|auto-fit, …)` остаётся одним
/// компонентом со своими именами (`repeat`). `None` — имён нет.
pub(in crate::style::computed) fn parse_line_names(v: &str) -> Option<gpui::GridAxisLineNames> {
    let tokens = line_name_tokens(v);
    let subgrid = tokens
        .first()
        .is_some_and(|t| t.eq_ignore_ascii_case("subgrid"));
    let mut out = gpui::GridAxisLineNames::default();
    let mut lines: Vec<Vec<String>> = if subgrid {
        Vec::new()
    } else {
        vec![Vec::new()]
    };
    let mut any = false;
    let mut after = false;
    for t in tokens.iter().skip(usize::from(subgrid)) {
        if let Some(names) = bracket_names(t) {
            any |= !names.is_empty();
            if subgrid {
                lines.push(names);
            } else if let Some(last) = lines.last_mut() {
                last.extend(names);
            }
            continue;
        }
        if let Some(inner) = t.strip_prefix("repeat(").and_then(|r| r.strip_suffix(')')) {
            let (count, rest) = inner.split_once(',')?;
            let body = repeat_body_names(rest, subgrid);
            any |= body.iter().any(|b| !b.is_empty());
            let count = count.trim();
            if count.eq_ignore_ascii_case("auto-fill") || count.eq_ignore_ascii_case("auto-fit") {
                if after {
                    return None;
                }
                out.before = std::mem::take(&mut lines);
                out.repeat = Some(body);
                lines = if subgrid {
                    Vec::new()
                } else {
                    vec![Vec::new()]
                };
                after = true;
            } else {
                let n: usize = count.parse().ok()?;
                for _ in 0..n.min(64) {
                    if subgrid {
                        lines.extend(body.iter().cloned());
                    } else {
                        if let (Some(last), Some(first)) = (lines.last_mut(), body.first()) {
                            last.extend(first.iter().cloned());
                        }
                        lines.extend(body.iter().skip(1).cloned());
                    }
                }
            }
            continue;
        }
        if !subgrid {
            lines.push(Vec::new());
        }
    }
    if !any {
        return None;
    }
    if after {
        out.after = lines;
    } else {
        out.before = lines;
    }
    Some(out)
}

/// Грань размещения по имени линии (css-grid-2 §8.3): `a`, `a 2`, `-1 a`,
/// `span a`, `span 2 a`. Без имени — `None` (числовую грань разбирает
/// `parse_placement`).
pub(in crate::style::computed) fn parse_named_placement(v: &str) -> Option<gpui::GridNamedLine> {
    let (mut span, mut num, mut name) = (false, None::<i16>, None::<String>);
    for t in v.split_whitespace() {
        if t.eq_ignore_ascii_case("span") {
            span = true;
        } else if let Ok(n) = t.parse::<i16>() {
            num = Some(n);
        } else if t.eq_ignore_ascii_case("auto") {
            return None;
        } else {
            name = Some(t.to_string());
        }
    }
    let name = name?;
    Some(if span {
        gpui::GridNamedLine::Span(name, num.unwrap_or(1).max(1) as u16)
    } else {
        gpui::GridNamedLine::Line(name, num.unwrap_or(0))
    })
}

/// Обе грани `grid-column`/`grid-row`: при одном значении-имени конец — то
/// же имя («if the first value is a <custom-ident>, the grid-row-end/
/// grid-column-end longhand is also set to that <custom-ident>», §8.4).
pub(in crate::style::computed) fn parse_named_pair(v: &str) -> [Option<gpui::GridNamedLine>; 2] {
    match v.split_once('/') {
        Some((a, b)) => [parse_named_placement(a), parse_named_placement(b)],
        None => {
            let start = parse_named_placement(v);
            let end = match &start {
                Some(gpui::GridNamedLine::Line(name, 0)) => {
                    Some(gpui::GridNamedLine::Line(name.clone(), 0))
                }
                _ => None,
            };
            [start, end]
        }
    }
}

pub(in crate::style::computed) fn parse_placement(v: &str) -> Placement {
    let v = v.trim();
    if let Some(n) = v.strip_prefix("span") {
        return n
            .trim()
            .parse()
            .map(Placement::Span)
            .unwrap_or(Placement::Auto);
    }
    v.parse().map(Placement::Line).unwrap_or(Placement::Auto)
}

/// `grid-column: 1 / 3` либо `span 2`.
pub(in crate::style::computed) fn parse_span(v: &str) -> Option<(Placement, Placement)> {
    match v.split_once('/') {
        Some((a, b)) => Some((parse_placement(a), parse_placement(b))),
        None => Some((parse_placement(v), Placement::Auto)),
    }
}
