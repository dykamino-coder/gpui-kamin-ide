//! Computed::apply_grid, хвост цепочки: grid-template-rows, grid-auto-*, размещение grid-row/column, grid-template-areas, grid-area. Ветви в исходном порядке после первых ветвей apply_grid.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_grid_placement(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "grid-template-rows" => {
                self.subgrid_rows = v.contains("subgrid");
                self.subgrid = self.subgrid_cols || self.subgrid_rows;
                self.grid_row_line_names = parse_line_names(v);
                self.grid_rows = parse_tracks(v);
            }
            "grid-auto-columns" => {
                // `grid-auto-columns: A B C` задаёт НЕСКОЛЬКО неявных дорожек,
                // и раскладка их циклит. Пока бралась первая, вторая колонка
                // получала ширину первой (`grid-support-grid-auto-columns-
                // rows-002`, `grid-floats-no-intrude-002`).
                let all = parse_tracks(v).unwrap_or_default();
                self.grid_auto_cols_list = if all.len() > 1 {
                    all.clone()
                } else {
                    Vec::new()
                };
                self.grid_auto_cols = all.into_iter().next();
            }
            "grid-auto-rows" => {
                let all = parse_tracks(v).unwrap_or_default();
                self.grid_auto_rows_list = if all.len() > 1 {
                    all.clone()
                } else {
                    Vec::new()
                };
                self.grid_auto_rows = all.into_iter().next();
            }
            "grid-auto-flow" => {
                let dense = v.contains("dense");
                self.grid_auto_flow = Some(match (v.contains("column"), dense) {
                    (true, true) => AutoFlow::ColDense,
                    (true, false) => AutoFlow::Col,
                    (false, true) => AutoFlow::RowDense,
                    (false, false) => AutoFlow::Row,
                })
            }
            "grid-column" => {
                self.grid_col = parse_span(v);
                self.grid_col_named = parse_named_pair(v);
            }
            "grid-row" => {
                self.grid_row = parse_span(v);
                self.grid_row_named = parse_named_pair(v);
            }
            "grid-column-start" => {
                let end = self.grid_col.map(|c| c.1).unwrap_or(Placement::Auto);
                self.grid_col = Some((parse_placement(v), end));
                self.grid_col_named[0] = parse_named_placement(v);
            }
            "grid-column-end" => {
                let start = self.grid_col.map(|c| c.0).unwrap_or(Placement::Auto);
                self.grid_col = Some((start, parse_placement(v)));
                self.grid_col_named[1] = parse_named_placement(v);
            }
            "grid-row-start" => {
                let end = self.grid_row.map(|c| c.1).unwrap_or(Placement::Auto);
                self.grid_row = Some((parse_placement(v), end));
                self.grid_row_named[0] = parse_named_placement(v);
            }
            "grid-row-end" => {
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
                let rows: Vec<Vec<String>> = v
                    .split('"')
                    .map(str::trim)
                    .filter(|r| !r.is_empty())
                    .map(|r| r.split_whitespace().map(str::to_string).collect())
                    .filter(|r: &Vec<String>| !r.is_empty())
                    .collect();
                self.grid_areas = (!rows.is_empty()).then_some(rows);
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
                    self.grid_row = Some((at(0), at(2)));
                    self.grid_col = Some((at(1), at(3)));
                } else if let Some(name) = parts.first().filter(|n| !n.is_empty()) {
                    // Одно значение — это ИМЯ области: номера линий для него
                    // знает только контейнер со своей раскладкой имён.
                    self.grid_area_name = Some((*name).to_string());
                }
            }
            _ => *hit = false,
        }
    }
}
