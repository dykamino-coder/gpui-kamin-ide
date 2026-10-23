//! Computed::apply_grid, продолжение цепочки: grid-template-columns, grid-lanes, пороги лунок
//! (сокращения grid/grid-template и устаревшие grid-*-gap раскрыты разбором блока). Ветви в исходном порядке; не совпавший ключ уходит в apply_grid_placement.

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
                self.subgrid_cols = super::subgrid::valid(v);
                self.subgrid = self.subgrid_cols || self.subgrid_rows;
                self.grid_col_line_names = parse_line_names(v);
                // `none` — ни одной явной дорожки (начальное значение).
                self.grid_cols = (v != "none").then(|| count_tracks(v)).flatten();
                self.grid_tracks = tracks;
                // Список без авто-повтора: поля повтора (ветвь `apply_grid`)
                // принадлежат этому же свойству и сбрасываются.
                self.grid_auto_fill_min = None;
                self.grid_auto_fill_tracks = Vec::new();
                self.auto_repeat_body_cols = None;
                self.auto_repeat_cols = None;
                if self.subgrid_cols || v == "none" {
                    self.accept_declaration();
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
            _ => self.apply_grid_placement(key, v, hit, vcx),
        }
    }
}

pub(super) fn template_tracks(v: &str, vcx: ValueCx<'_>) -> Option<Option<Vec<TrackSize>>> {
    if v.trim() == "none" || super::subgrid::valid(v) {
        Some(None)
    } else {
        parse_tracks(v, vcx).map(Some)
    }
}
