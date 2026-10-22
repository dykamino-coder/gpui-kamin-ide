//! Сокращения сетки, раскрытые при разборе (css-grid-2 §7.4 `grid-template`,
//! §7.8 `grid`, §8.4 `grid-row`/`grid-column`/`grid-area`). Части идут на
//! места `ShorthandId::longhands`; каждую проверяет грамматика своего
//! свойства.

use super::{LonghandId, ShorthandId};
use LonghandId::*;
use tokens::tokens;

pub(super) const GRID_ROW: &[LonghandId] = &[GridRowStart, GridRowEnd];
pub(super) const GRID_COLUMN: &[LonghandId] = &[GridColumnStart, GridColumnEnd];
pub(super) const GRID_AREA: &[LonghandId] =
    &[GridRowStart, GridColumnStart, GridRowEnd, GridColumnEnd];
pub(super) const GRID_TEMPLATE: &[LonghandId] =
    &[GridTemplateRows, GridTemplateColumns, GridTemplateAreas];
/// `grid` сбрасывает и неявную сетку, но не промежутки (css-grid-2 §7.8).
pub(super) const GRID: &[LonghandId] = &[
    GridTemplateRows,
    GridTemplateColumns,
    GridTemplateAreas,
    GridAutoRows,
    GridAutoColumns,
    GridAutoFlow,
];

impl ShorthandId {
    pub(super) fn grid_longhands(self) -> Option<&'static [LonghandId]> {
        Some(match self {
            Self::GridRow => GRID_ROW,
            Self::GridColumn => GRID_COLUMN,
            Self::GridArea => GRID_AREA,
            Self::GridTemplate => GRID_TEMPLATE,
            Self::Grid => GRID,
            _ => return None,
        })
    }

    pub(super) fn grid_parts(self, value: &str) -> Option<Vec<String>> {
        match self {
            Self::GridRow | Self::GridColumn => lines(value, 2),
            Self::GridArea => lines(value, 4),
            Self::GridTemplate => grid_template(value),
            _ => grid(value),
        }
    }
}

/// Одиночный `<custom-ident>` грани: его копирует опущенная грань.
fn custom_ident(t: &str) -> bool {
    let mut words = t.split_whitespace();
    let (Some(w), None) = (words.next(), words.next()) else {
        return false;
    };
    !w.eq_ignore_ascii_case("auto")
        && !w.eq_ignore_ascii_case("span")
        && w.parse::<i32>().is_err()
        && crate::style::css::selector_tokens::ident(w)
}

/// `grid-row`/`grid-column: <grid-line> [ / <grid-line> ]?` и `grid-area:
/// <grid-line> [ / <grid-line> ]{0,3}` (css-grid-2 §8.4). Опущенная грань —
/// копия `<custom-ident>` парной грани (конец — начала той же оси, начало
/// колонки — начала ряда), иначе `auto`. Порядок частей у `grid-area`:
/// начало ряда, начало колонки, конец ряда, конец колонки.
fn lines(value: &str, max: usize) -> Option<Vec<String>> {
    let parts: Vec<String> = value.split('/').map(|p| p.trim().to_string()).collect();
    if parts.len() > max || parts.iter().any(String::is_empty) {
        return None;
    }
    let copy = |t: &String| {
        if custom_ident(t) {
            t.clone()
        } else {
            "auto".to_string()
        }
    };
    if max == 2 {
        let end = parts.get(1).cloned().unwrap_or_else(|| copy(&parts[0]));
        return Some(vec![parts[0].clone(), end]);
    }
    let row_start = parts[0].clone();
    let col_start = parts.get(1).cloned().unwrap_or_else(|| copy(&row_start));
    let row_end = parts.get(2).cloned().unwrap_or_else(|| copy(&row_start));
    let col_end = parts.get(3).cloned().unwrap_or_else(|| copy(&col_start));
    Some(vec![row_start, col_start, row_end, col_end])
}

fn is_string(t: &str) -> bool {
    t.starts_with(['"', '\''])
}

fn is_names(t: &str) -> bool {
    t.starts_with('[')
}

/// Имена двух соседних групп одной линии — одной группой.
fn merge_names(a: &str, b: &str) -> String {
    let inner = |t: &str| t[1..t.len() - 1].trim().to_string();
    format!("[{} {}]", inner(a), inner(b))
}

/// `grid-template: none | [ <'grid-template-rows'> / <'grid-template-columns'> ]
/// | [ <line-names>? <string> <track-size>? <line-names>? ]+ [ /
/// <explicit-track-list> ]?` (css-grid-2 §7.4). Части: ряды, колонки,
/// области. В форме со строками ряды собираются из размеров (опущенный —
/// `auto`) и имён (соседние группы одной линии сливаются), строки — области.
fn grid_template(value: &str) -> Option<Vec<String>> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("none") {
        return Some(vec!["none".into(), "none".into(), "none".into()]);
    }
    let toks = tokens(v)?;
    let slash = toks.iter().position(|t| t == "/");
    if toks.iter().filter(|t| *t == "/").count() > 1 {
        return None;
    }
    let (left, right) = match slash {
        Some(i) => (&toks[..i], Some(&toks[i + 1..])),
        None => (&toks[..], None),
    };
    if !left.iter().any(|t| is_string(t)) {
        let (rows, cols) = (left.join(" "), right?.join(" "));
        if rows.is_empty() || cols.is_empty() {
            return None;
        }
        return Some(vec![rows, cols, "none".into()]);
    }
    let cols = match right {
        Some([]) => return None,
        Some(r) => {
            let text = r.join(" ");
            // `<explicit-track-list>`: без авто-повтора.
            let lower = text.to_ascii_lowercase();
            if lower.contains("auto-fill") || lower.contains("auto-fit") {
                return None;
            }
            text
        }
        None => "none".into(),
    };
    let mut rows: Vec<String> = Vec::new();
    let mut areas: Vec<String> = Vec::new();
    let mut i = 0;
    while i < left.len() {
        // Имена перед строкой — на той же линии, что и хвост прошлого ряда.
        if is_names(&left[i]) {
            match rows.last_mut() {
                Some(last) if is_names(last) => *last = merge_names(last, &left[i]),
                _ => rows.push(left[i].clone()),
            }
            i += 1;
        }
        let s = left.get(i).filter(|t| is_string(t))?;
        areas.push(s.clone());
        i += 1;
        match left.get(i) {
            Some(t) if !is_string(t) && !is_names(t) => {
                if t.to_ascii_lowercase().starts_with("repeat(") {
                    return None;
                }
                rows.push(t.clone());
                i += 1;
            }
            _ => rows.push("auto".into()),
        }
        if let Some(t) = left.get(i).filter(|t| is_names(t)) {
            rows.push(t.clone());
            i += 1;
        }
    }
    Some(vec![rows.join(" "), cols, areas.join(" ")])
}

/// Сторона `[ auto-flow && dense? ] <неявные дорожки>?`: (`dense`, дорожки).
fn auto_flow_side(side: &[String]) -> Option<(bool, String)> {
    let (mut flow, mut dense, mut i) = (false, false, 0);
    while let Some(t) = side.get(i) {
        match t.to_ascii_lowercase().as_str() {
            "auto-flow" if !flow => flow = true,
            "dense" if !dense => dense = true,
            _ => break,
        }
        i += 1;
    }
    if !flow {
        return None;
    }
    let rest = side[i..].join(" ");
    Some((dense, if rest.is_empty() { "auto".into() } else { rest }))
}

/// `grid: <'grid-template'> | <'grid-template-rows'> / [ auto-flow && dense? ]
/// <'grid-auto-columns'>? | [ auto-flow && dense? ] <'grid-auto-rows'>? /
/// <'grid-template-columns'>` (css-grid-2 §7.8). Неявные дорожки и автопоток
/// сбрасываются (опущенные — начальные), промежутки — нет.
fn grid(value: &str) -> Option<Vec<String>> {
    let toks = tokens(value.trim())?;
    let has_flow = |s: &[String]| s.iter().any(|t| t.eq_ignore_ascii_case("auto-flow"));
    let slash = toks.iter().position(|t| t == "/");
    let (left, right) = match slash {
        Some(i) => (&toks[..i], &toks[i + 1..]),
        None => (&toks[..], &toks[..0]),
    };
    let none = || "none".to_string();
    let auto = || "auto".to_string();
    match (has_flow(left), slash.is_some() && has_flow(right)) {
        (false, false) => {
            let mut parts = grid_template(value)?;
            parts.extend([auto(), auto(), "row".into()]);
            Some(parts)
        }
        (true, false) => {
            let (dense, auto_rows) = auto_flow_side(left)?;
            let cols = right.join(" ");
            if slash.is_none() || cols.is_empty() {
                return None;
            }
            let flow = if dense { "row dense" } else { "row" };
            Some(vec![none(), cols, none(), auto_rows, auto(), flow.into()])
        }
        (false, true) => {
            let (dense, auto_cols) = auto_flow_side(right)?;
            let rows = left.join(" ");
            if rows.is_empty() {
                return None;
            }
            let flow = if dense { "column dense" } else { "column" };
            Some(vec![rows, none(), none(), auto(), auto_cols, flow.into()])
        }
        (true, true) => None,
    }
}

mod tokens;

#[cfg(test)]
mod tests;
