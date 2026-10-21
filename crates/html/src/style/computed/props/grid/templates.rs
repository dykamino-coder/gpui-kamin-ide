//! Computed::apply_grid, продолжение цепочки: сокращения grid/grid-template, grid-lanes, пороги
//! лунок, grid-*-gap. Ветви в исходном порядке; не совпавший ключ уходит в apply_grid_placement.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_grid_templates(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            // Проверено: полный разбор ИМЁН ЛИНИЙ (имена списка дорожек с
            // раскрытием счётного `repeat`, именованная грань `span a`/`a -1`/
            // голое имя с поиском `имя-start`/`имя-end`, поле `NamedEdge`
            // рядом с `Placement`, разрешитель имён в номера линий с неявными
            // именами от `grid-template-areas` для сетки и лунок) ничего не
            // меняет: пары с именами линий держат не имена —
            // `column-line-names-011` подсетка, `-016` имена внутри
            // `repeat(auto-fill, …)`, `-003` красна и при верно разрешённых
            // гранях. Вводить вместе с подсеткой и именами в авто-повторе.
            "grid-template-columns" => {
                // Признак ПОСЛЕДНЕГО объявления, а не накопленный: каскад
                // берёт последнее (`grid-lanes-subgrid-001b`: правило класса
                // `grid: subgrid / subgrid` и встроенное `grid: auto/subgrid`
                // — ряды у подсетки СВОИ, а ИЛИ оставлял их подсеточными, и
                // с настоящей подсеткой taffy ряд сжимался в один).
                let tracks = parsed_value!(self, template_tracks(v, vcx));
                self.subgrid_cols = v.contains("subgrid");
                self.subgrid = self.subgrid_cols || self.subgrid_rows;
                self.grid_col_line_names = parse_line_names(v);
                self.grid_cols = count_tracks(v);
                self.grid_tracks = tracks;
                if self.subgrid_cols || v == "none" {
                    self.accept_declaration();
                }
            }
            // `grid: <ряды> / <колонки>` и `grid-template: <ряды> / <колонки>`
            // — самая частая короткая запись сетки в тестах и в вёрстке.
            // Формы с `auto-flow` описывают неявные дорожки: там сторона со
            // словом задаёт направление автопотока, а вторая — шаблон.
            "grid" | "grid-template" => {
                let (rows, cols) = split_slash(v);
                if !shorthand_tracks_valid(rows, vcx)
                    || (!cols.is_empty() && !shorthand_tracks_valid(cols, vcx))
                {
                    return;
                }
                // `grid: subgrid / subgrid`, `grid-template: subgrid / 20% 30%`
                // — слово стоит на СВОЕЙ стороне косой черты, и ось у него
                // своя. Без косой черты `split_slash` кладёт всё в `rows`, что
                // и верно: сокращение начинается с рядов. Сокращение задаёт
                // обе оси заново (см. `grid-template-columns`).
                self.subgrid_rows = rows.contains("subgrid");
                self.subgrid_cols = cols.contains("subgrid");
                self.subgrid = self.subgrid_cols || self.subgrid_rows;
                match (rows.contains("auto-flow"), cols.contains("auto-flow")) {
                    (true, _) => {
                        self.grid_auto_flow = Some(if rows.contains("dense") {
                            AutoFlow::RowDense
                        } else {
                            AutoFlow::Row
                        });
                        self.apply_one("grid-auto-rows", strip_auto_flow(rows), vcx);
                        self.apply_one("grid-template-columns", cols, vcx);
                    }
                    (_, true) => {
                        self.grid_auto_flow = Some(if cols.contains("dense") {
                            AutoFlow::ColDense
                        } else {
                            AutoFlow::Col
                        });
                        self.apply_one("grid-template-rows", rows, vcx);
                        self.apply_one("grid-auto-columns", strip_auto_flow(cols), vcx);
                    }
                    _ => {
                        self.apply_one("grid-template-rows", rows, vcx);
                        if !cols.is_empty() {
                            self.apply_one("grid-template-columns", cols, vcx);
                        }
                    }
                }
            }
            // Значение бывает составным: `row fill-reverse`, `column
            // track-reverse`. Сверка со строкой ЦЕЛИКОМ путала ось на каждом
            // таком тесте.
            "grid-lanes-pack" => {
                self.lanes_dense = parsed_value!(
                    self,
                    matches!(v, "pack" | "dense" | "next").then_some(v == "dense")
                )
            }
            // Порог «равенства» лунок при авто-выборе (css-grid-3): лунки с
            // разницей заполнения меньше порога считаются равными и берутся в
            // ПОРЯДКЕ ДОКУМЕНТА. `normal` (дефолт!) = 1em, `infinite` —
            // строгий порядок укладки безотносительно высот.
            "flow-tolerance" | "item-tolerance" => {
                self.lanes_tolerance = parsed_value!(
                    self,
                    match v.trim() {
                        "normal" => Some(None),
                        "infinite" => Some(Some(Len::Px(f32::INFINITY))),
                        t => Len::parse(t, vcx).map(Some),
                    }
                );
                if v == "normal" {
                    self.accept_declaration();
                }
            }
            // Первая часть — ось лунок, дальше — реверсы: `fill-reverse`
            // заполняет лунки с другого конца, `track-reverse` перечисляет
            // сами лунки в обратном порядке (css-grid-3).
            "grid-lanes-direction" => {
                let words: Vec<_> = v.split_whitespace().collect();
                if words.is_empty()
                    || !matches!(words[0], "row" | "column")
                    || words[1..]
                        .iter()
                        .any(|w| !matches!(*w, "fill-reverse" | "track-reverse"))
                {
                    return;
                }
                self.accept_declaration();
                self.lanes_row = Some(v.split_whitespace().next() == Some("row"));
                self.lanes_fill_reverse = v.split_whitespace().any(|w| w == "fill-reverse");
                self.lanes_track_reverse = v.split_whitespace().any(|w| w == "track-reverse");
            }
            "grid-gap" => self.apply_one("gap", v, vcx),
            "grid-row-gap" => self.apply_one("row-gap", v, vcx),
            "grid-column-gap" => self.apply_one("column-gap", v, vcx),
            _ => self.apply_grid_placement(key, v, hit, vcx),
        }
    }
}

pub(super) fn template_tracks(v: &str, vcx: ValueCx<'_>) -> Option<Option<Vec<TrackSize>>> {
    if v == "none" || v.trim_start().starts_with("subgrid") {
        Some(None)
    } else {
        parse_tracks(v, vcx).map(Some)
    }
}

// Сторона auto-flow без размера означает auto. Строки областей сами не дорожки.
fn shorthand_tracks_valid(v: &str, vcx: ValueCx<'_>) -> bool {
    let tracks = strip_auto_flow(v);
    if tracks.is_empty() && v.contains("auto-flow") {
        return true;
    }
    let mut quote = None;
    let mut sizes = String::new();
    let mut areas = false;
    for c in tracks.chars() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if matches!(c, '\'' | '"') {
            quote = Some(c);
            areas = true;
            sizes.push(' ');
        } else {
            sizes.push(c);
        }
    }
    quote.is_none()
        && ((areas && sizes.trim().is_empty()) || template_tracks(&sizes, vcx).is_some())
}
