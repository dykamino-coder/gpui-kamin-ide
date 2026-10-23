//! Computed::apply_grid, хвост цепочки: grid-template-rows, grid-auto-*, грани размещения
//! grid-row-start … grid-column-end, grid-template-areas (сокращения grid-row/-column/-area
//! раскрыты разбором блока). Ветви в исходном порядке после первых ветвей apply_grid.

use super::*;
use crate::document::state::ValueCx;

pub(super) mod values;
use values::placement_list;

impl Computed {
    pub(super) fn apply_grid_placement(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "grid-template-rows" => {
                let tracks = parsed_value!(self, super::templates::template_tracks(v, vcx));
                self.subgrid_rows = super::subgrid::valid(v, vcx);
                self.subgrid = self.subgrid_cols || self.subgrid_rows;
                self.grid_row_line_names = if self.subgrid_rows {
                    super::subgrid::parse(v, vcx)
                } else {
                    parse_line_names(v)
                };
                self.grid_rows = tracks;
                // Поля авто-повтора рядов принадлежат этому же свойству.
                self.grid_auto_fill_row = None;
                self.auto_repeat_body_rows = None;
                self.auto_repeat_rows = None;
                if self.subgrid_rows || v == "none" {
                    self.accept_declaration();
                }
            }
            "grid-auto-columns" => {
                // `grid-auto-columns: A B C` задаёт НЕСКОЛЬКО неявных дорожек,
                // и раскладка их циклит. Пока бралась первая, вторая колонка
                // получала ширину первой (`grid-support-grid-auto-columns-
                // rows-002`, `grid-floats-no-intrude-002`).
                let all =
                    apply_parsed!(self, super::values::auto_tracks(v, vcx)).unwrap_or_default();
                self.grid_auto_cols_list = if all.len() > 1 {
                    all.clone()
                } else {
                    Vec::new()
                };
                self.grid_auto_cols = all.into_iter().next();
            }
            "grid-auto-rows" => {
                let all =
                    apply_parsed!(self, super::values::auto_tracks(v, vcx)).unwrap_or_default();
                self.grid_auto_rows_list = if all.len() > 1 {
                    all.clone()
                } else {
                    Vec::new()
                };
                self.grid_auto_rows = all.into_iter().next();
            }
            "grid-auto-flow" => {
                self.grid_auto_flow = apply_parsed!(self, super::values::auto_flow(v));
            }
            "grid-row-start" | "grid-row-end" | "grid-column-start" | "grid-column-end" => {
                if !placement_list(v, 1) {
                    return;
                }
                self.accept_declaration();
                self.set_grid_line(key, v);
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
                let rows = apply_parsed!(self, super::values::areas(v));
                self.grid_areas = rows.filter(|r| !r.is_empty());
            }
            _ => *hit = false,
        }
    }
}

impl Computed {
    /// Одна грань размещения (`grid-row-start` … `grid-column-end`, css-grid-2
    /// §8.3): своя половина линий и имён. Ось без единой заданной грани —
    /// `None`, как у начального `auto / auto`. Имя области для раскладки —
    /// вид вычисленных граней (`values::area_name`).
    fn set_grid_line(&mut self, key: &str, v: &str) {
        let (row, end) = match key {
            "grid-row-start" => (true, false),
            "grid-row-end" => (true, true),
            "grid-column-start" => (false, false),
            _ => (false, true),
        };
        let (lines, named) = if row {
            (&mut self.grid_row, &mut self.grid_row_named)
        } else {
            (&mut self.grid_col, &mut self.grid_col_named)
        };
        let mut pair = lines.unwrap_or((Placement::Auto, Placement::Auto));
        let i = usize::from(end);
        if end {
            pair.1 = parse_placement(v);
        } else {
            pair.0 = parse_placement(v);
        }
        named[i] = parse_named_placement(v);
        let unset = pair.0 == Placement::Auto
            && pair.1 == Placement::Auto
            && named.iter().all(Option::is_none);
        *lines = (!unset).then_some(pair);
        self.grid_area_name = super::values::area_name(&self.grid_row_named, &self.grid_col_named);
    }
}
