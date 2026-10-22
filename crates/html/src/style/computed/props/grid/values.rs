//! Значения длинных свойств сетки реестра (css-grid-2 §7-§8): ОДИН разбор на
//! свойство — им реестр проверяет объявление, им же обработчик пишет поля
//! стиля.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::css::LonghandId;

/// Годится ли значение длинному свойству сетки.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    use LonghandId::*;
    let v = value.trim();
    match id {
        GridTemplateRows | GridTemplateColumns => template(v, vcx),
        GridTemplateAreas => areas(v).is_some(),
        GridAutoRows | GridAutoColumns => auto_tracks(v, vcx).is_some(),
        GridAutoFlow => auto_flow(v).is_some(),
        GridRowStart | GridRowEnd | GridColumnStart | GridColumnEnd => {
            super::placement::values::placement_list(v, 1)
        }
        _ => false,
    }
}

/// `grid-template-rows`/`-columns: none | <track-list> | <auto-track-list> |
/// subgrid <line-name-list>?` (css-grid-2 §7.2). Вокруг авто-повтора — только
/// `<fixed-size>` (`auto_repeat_outside_intrinsic`).
pub(crate) fn template(v: &str, vcx: ValueCx<'_>) -> bool {
    let auto_repeat = (v.contains("auto-fill") || v.contains("auto-fit"))
        && !v.trim_start().starts_with("subgrid");
    if auto_repeat {
        !auto_repeat_outside_intrinsic(v) && parse_tracks(v, vcx).is_some()
    } else {
        super::templates::template_tracks(v, vcx).is_some()
    }
}

/// `grid-auto-rows`/`-columns: <track-size>+` (css-grid-2 §7.6): без имён
/// линий и без `repeat()`.
pub(crate) fn auto_tracks(v: &str, vcx: ValueCx<'_>) -> Option<Vec<TrackSize>> {
    if v.contains('[') || v.to_ascii_lowercase().contains("repeat(") {
        return None;
    }
    parse_tracks(v, vcx)
}

/// `grid-auto-flow: [ row | column ] || dense` (css-grid-2 §7.7).
pub(crate) fn auto_flow(v: &str) -> Option<AutoFlow> {
    let words: Vec<String> = v.split_whitespace().map(str::to_ascii_lowercase).collect();
    let (mut axis, mut dense) = (None, false);
    if words.is_empty() || words.len() > 2 {
        return None;
    }
    for w in &words {
        match w.as_str() {
            "row" | "column" if axis.is_none() => axis = Some(w == "column"),
            "dense" if !dense => dense = true,
            _ => return None,
        }
    }
    Some(match (axis == Some(true), dense) {
        (true, true) => AutoFlow::ColDense,
        (true, false) => AutoFlow::Col,
        (false, true) => AutoFlow::RowDense,
        (false, false) => AutoFlow::Row,
    })
}

/// Ряды `grid-template-areas: none | <string>+` (css-grid-2 §7.3); `none` —
/// пустой список. Строка делится на ячейки (§7.3.1): последовательность
/// символов имени — именованная ячейка, точки подряд — пустая, иное
/// (мусор) делает объявление негодным. Ряды одной длины, у каждого имени —
/// заполненный прямоугольник.
pub(crate) fn areas(v: &str) -> Option<Vec<Vec<String>>> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let mut rows = Vec::new();
    let mut rest = v;
    while let Some(quote) = rest.chars().next() {
        if quote != '"' && quote != '\'' {
            return None;
        }
        let len = crate::style::css::skip_string(&rest[1..], quote);
        let body = rest[1..1 + len].strip_suffix(quote)?;
        rows.push(area_cells(body)?);
        rest = rest[1 + len..].trim_start();
    }
    let width = rows.first()?.len();
    if width == 0 || rows.iter().any(|r| r.len() != width) {
        return None;
    }
    rectangular(&rows).then_some(rows)
}

/// Ячейки одной строки областей.
fn area_cells(body: &str) -> Option<Vec<String>> {
    let name = |c: char| c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii();
    let mut cells = Vec::new();
    let mut chars = body.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let run = |chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
                   f: &dyn Fn(char) -> bool| {
            let mut end = i + c.len_utf8();
            while let Some(&(j, d)) = chars.peek()
                && f(d)
            {
                end = j + d.len_utf8();
                chars.next();
            }
            end
        };
        if c.is_whitespace() {
            continue;
        } else if c == '.' {
            let end = run(&mut chars, &|d| d == '.');
            cells.push(body[i..end].to_string());
        } else if name(c) {
            let end = run(&mut chars, &name);
            cells.push(body[i..end].to_string());
        } else {
            return None;
        }
    }
    Some(cells)
}

/// Каждое имя занимает ровно прямоугольник своих крайних ячеек.
fn rectangular(rows: &[Vec<String>]) -> bool {
    let mut seen: Vec<&str> = Vec::new();
    for row in rows {
        for cell in row {
            if cell.starts_with('.') || seen.contains(&cell.as_str()) {
                continue;
            }
            seen.push(cell);
            let (mut r0, mut r1, mut c0, mut c1, mut n) = (usize::MAX, 0, usize::MAX, 0, 0);
            for (r, cells) in rows.iter().enumerate() {
                for (c, x) in cells.iter().enumerate() {
                    if x == cell {
                        r0 = r0.min(r);
                        r1 = r1.max(r + 1);
                        c0 = c0.min(c);
                        c1 = c1.max(c + 1);
                        n += 1;
                    }
                }
            }
            if n != (r1 - r0) * (c1 - c0)
                || !(r0..r1).all(|r| (c0..c1).all(|c| rows[r][c] == *cell))
            {
                return false;
            }
        }
    }
    true
}

/// Имя области по вычисленным граням размещения: `grid-area: <ident>` — все
/// четыре грани один и тот же `<custom-ident>` (css-grid-2 §8.4). Раскладка
/// находит такую область по `grid-template-areas` контейнера.
pub(crate) fn area_name(
    row: &[Option<gpui::GridNamedLine>; 2],
    col: &[Option<gpui::GridNamedLine>; 2],
) -> Option<String> {
    let Some(gpui::GridNamedLine::Line(name, 0)) = &row[0] else {
        return None;
    };
    row.iter()
        .chain(col.iter())
        .all(|n| matches!(n, Some(gpui::GridNamedLine::Line(m, 0)) if m == name))
        .then(|| name.clone())
}

#[cfg(test)]
mod tests {
    use super::areas;

    #[test]
    fn areas_accept_both_quote_kinds_and_null_runs() {
        let rows = areas("'a a b' \"c ... b\"").unwrap();
        assert_eq!(rows, vec![vec!["a", "a", "b"], vec!["c", "...", "b"]]);
        assert_eq!(areas("none"), Some(Vec::new()));
        assert_eq!(areas("'a.b'").unwrap(), vec![vec!["a", ".", "b"]]);
    }

    #[test]
    fn areas_reject_ragged_rows_non_rectangles_and_trash() {
        assert!(areas("'a b' 'c'").is_none());
        assert!(areas("'a b' 'b a'").is_none());
        assert!(areas("'a a' 'a .'").is_none());
        assert!(areas("'a $'").is_none());
        assert!(areas("''").is_none());
    }
}
