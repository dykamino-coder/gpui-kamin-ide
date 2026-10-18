//! Computed::apply_grid, хвост цепочки: grid-template-rows, grid-auto-*, размещение grid-row/column, grid-template-areas, grid-area. Ветви в исходном порядке после первых ветвей apply_grid.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_grid_placement(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "grid-template-rows" => {
                self.subgrid_rows = v.contains("subgrid");
                self.subgrid = self.subgrid_cols || self.subgrid_rows;
                self.grid_row_line_names = parse_line_names(v);
                self.grid_rows = record_parsed!(self, parse_tracks(v, vcx));
                if self.subgrid_rows || v == "none" {
                    self.accept_declaration();
                }
            }
            "grid-auto-columns" => {
                // `grid-auto-columns: A B C` задаёт НЕСКОЛЬКО неявных дорожек,
                // и раскладка их циклит. Пока бралась первая, вторая колонка
                // получала ширину первой (`grid-support-grid-auto-columns-
                // rows-002`, `grid-floats-no-intrude-002`).
                let all = record_parsed!(self, parse_tracks(v, vcx)).unwrap_or_default();
                self.grid_auto_cols_list = if all.len() > 1 {
                    all.clone()
                } else {
                    Vec::new()
                };
                self.grid_auto_cols = all.into_iter().next();
            }
            "grid-auto-rows" => {
                let all = record_parsed!(self, parse_tracks(v, vcx)).unwrap_or_default();
                self.grid_auto_rows_list = if all.len() > 1 {
                    all.clone()
                } else {
                    Vec::new()
                };
                self.grid_auto_rows = all.into_iter().next();
            }
            "grid-auto-flow" => {
                self.accept_declaration();
                let dense = v.contains("dense");
                self.grid_auto_flow = Some(match (v.contains("column"), dense) {
                    (true, true) => AutoFlow::ColDense,
                    (true, false) => AutoFlow::Col,
                    (false, true) => AutoFlow::RowDense,
                    (false, false) => AutoFlow::Row,
                })
            }
            "grid-column" => {
                self.grid_col = record_parsed!(self, parse_span(v));
                self.grid_col_named = parse_named_pair(v);
            }
            "grid-row" => {
                self.grid_row = record_parsed!(self, parse_span(v));
                self.grid_row_named = parse_named_pair(v);
            }
            "grid-column-start" => {
                self.accept_declaration();
                let end = self.grid_col.map(|c| c.1).unwrap_or(Placement::Auto);
                self.grid_col = Some((parse_placement(v), end));
                self.grid_col_named[0] = parse_named_placement(v);
            }
            "grid-column-end" => {
                self.accept_declaration();
                let start = self.grid_col.map(|c| c.0).unwrap_or(Placement::Auto);
                self.grid_col = Some((start, parse_placement(v)));
                self.grid_col_named[1] = parse_named_placement(v);
            }
            "grid-row-start" => {
                self.accept_declaration();
                let end = self.grid_row.map(|c| c.1).unwrap_or(Placement::Auto);
                self.grid_row = Some((parse_placement(v), end));
                self.grid_row_named[0] = parse_named_placement(v);
            }
            "grid-row-end" => {
                self.accept_declaration();
                let start = self.grid_row.map(|c| c.0).unwrap_or(Placement::Auto);
                self.grid_row = Some((start, parse_placement(v)));
                self.grid_row_named[1] = parse_named_placement(v);
            }
            "grid-template-areas" => {
                // Явное `inherit` — запись родителя (её переносит
                // `doc::settle_explicit_inherit`): прежде слово само шло в
                // область `inherit`, а `grid-area: a` у детей не находил
                // области (`grid-placement-using-named-grid-lines-008`).
                self.grid_areas_inherit = v.trim().eq_ignore_ascii_case("inherit");
                if self.grid_areas_inherit {
                    return;
                }
                // Каждая строка записи — ряд сетки: `"head head" "side main"`.
                let rows = area_rows(v);
                self.grid_areas = record_parsed!(self, (!rows.is_empty()).then_some(rows));
                if v == "none" {
                    self.accept_declaration();
                }
            }
            // `grid-area: строка / колонка / конец строки / конец колонки`.
            "grid-area" => {
                let parts: Vec<&str> = v.split('/').map(str::trim).collect();
                let at = |i: usize| {
                    parts
                        .get(i)
                        .map(|p| parse_placement(p))
                        .unwrap_or(Placement::Auto)
                };
                // Именованные грани (css-grid-2 §8.4 `grid-area`): опущенная
                // грань повторяет имя противоположной по оси стороны
                // (`grid-area: a` — все четыре грани `a`).
                let named = |i: usize| parts.get(i).and_then(|p| parse_named_placement(p));
                let ident = |n: &Option<gpui::GridNamedLine>| match n {
                    Some(gpui::GridNamedLine::Line(name, 0)) => {
                        Some(gpui::GridNamedLine::Line(name.clone(), 0))
                    }
                    _ => None,
                };
                let row_start = named(0);
                let col_start = if parts.len() > 1 {
                    named(1)
                } else {
                    ident(&row_start)
                };
                let row_end = if parts.len() > 2 {
                    named(2)
                } else {
                    ident(&row_start)
                };
                let col_end = if parts.len() > 3 {
                    named(3)
                } else {
                    ident(&col_start)
                };
                self.grid_row_named = [row_start, row_end];
                self.grid_col_named = [col_start, col_end];
                if parts.len() >= 2 {
                    self.accept_declaration();
                    self.grid_row = Some((at(0), at(2)));
                    self.grid_col = Some((at(1), at(3)));
                } else if let Some(name) = parts.first().filter(|n| !n.is_empty()) {
                    self.accept_declaration();
                    // Одно значение — это ИМЯ области: номера линий для него
                    // знает только контейнер со своей раскладкой имён.
                    self.grid_area_name = Some((*name).to_string());
                }
            }
            _ => *hit = false,
        }
    }
}

/// Ряды `grid-template-areas`: каждая CSS-строка (`"…"` или `'…'`, css-grid-2
/// §7.3 «<string>+») — ряд, её ячейки разделены пробелами. Одинарные кавычки
/// прежде не снимались: `'a a a'` давал области `'a`, `a`, `a'`
/// (`grid-subgridded-to-grid-lanes/line-names/column-line-names-010`).
fn area_rows(v: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut rest = v.trim_start();
    while let Some(quote) = rest.chars().next() {
        if quote != '"' && quote != '\'' {
            // Не строка — не ряд (`none` и мусор рядов не дают).
            let skip = rest.find(['"', '\'']).unwrap_or(rest.len());
            rest = rest[skip..].trim_start();
            continue;
        }
        let len = crate::style::css::skip_string(&rest[1..], quote);
        let body = rest[1..1 + len]
            .strip_suffix(quote)
            .unwrap_or(&rest[1..1 + len]);
        let row: Vec<String> = body.split_whitespace().map(str::to_string).collect();
        if !row.is_empty() {
            rows.push(row);
        }
        rest = rest[1 + len..].trim_start();
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::area_rows;

    #[test]
    fn area_rows_accept_both_quote_kinds() {
        let rows = area_rows("'a a b' \"c . d\"");
        assert_eq!(rows, vec![vec!["a", "a", "b"], vec!["c", ".", "d"]]);
        assert!(area_rows("none").is_empty());
    }
}
