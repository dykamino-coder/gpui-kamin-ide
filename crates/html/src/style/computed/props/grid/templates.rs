//! Computed::apply_grid, продолжение цепочки: сокращения grid/grid-template, grid-lanes, пороги лунок, grid-*-gap. Ветви в исходном порядке; не совпавший ключ уходит в apply_grid_placement.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_grid_templates(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: ИМЕНА ЛИНИЙ целиком (план — в
            // `target/scout-linenames.md`, шаги A1-A3). Написано и работало:
            // разбор имён списка дорожек (`[a] 50px 50px [a] 50px 50px [a]` →
            // `[["a"],[],["a"],[],["a"]]`, с раскрытием счётного `repeat`),
            // разбор именованной грани (`span a`, `a -1`, голое имя с поиском
            // `имя-start`/`имя-end`), поле `NamedEdge` рядом с `Placement`
            // (чтобы тот остался `Copy`), разрешитель имён в номера линий с
            // неявными именами от `grid-template-areas`, вызванный и для
            // классической сетки, и для лунок. Печатью подтверждено, что
            // разрешитель ДОХОДИТ до контейнера с именами и разрешает грани.
            // Срез css-grid (1133 пары, 646 зелёных): 646 — ноль приобретено,
            // ноль потеряно; поимённо не сдвинулась НИ ОДНА из 33 пар с
            // именами линий, а `grid-lanes-grid-placement-named-lines-001/002`
            // ушли 16.30 → 17.92 и 14.11 → 14.54.
            // Значит эти пары держат не имена: `column-line-names-011` —
            // субсетка (шаг B), `-016` — имена внутри `repeat(auto-fill, …)`
            // (шаг C), а `-003` при верно разрешённых гранях (span a / a -1 →
            // линии 3..5) остаётся на 0.52. Возвращать вместе с шагами B и C.
            "grid-template-columns" => {
                // Признак ПОСЛЕДНЕГО объявления, а не накопленный: каскад
                // берёт последнее (`grid-lanes-subgrid-001b`: правило класса
                // `grid: subgrid / subgrid` и встроенное `grid: auto/subgrid`
                // — ряды у подсетки СВОИ, а ИЛИ оставлял их подсеточными, и
                // с настоящей подсеткой taffy ряд сжимался в один).
                self.subgrid_cols = v.contains("subgrid");
                self.subgrid = self.subgrid_cols || self.subgrid_rows;
                self.grid_col_line_names = parse_line_names(v);
                self.grid_cols = count_tracks(v);
                self.grid_tracks = parse_tracks(v);
            }
            // `grid: <ряды> / <колонки>` и `grid-template: <ряды> / <колонки>`
            // — самая частая короткая запись сетки в тестах и в вёрстке.
            // Формы с `auto-flow` описывают неявные дорожки: там сторона со
            // словом задаёт направление автопотока, а вторая — шаблон.
            "grid" | "grid-template" => {
                let (rows, cols) = split_slash(v);
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
                        self.apply_one("grid-auto-rows", strip_auto_flow(rows));
                        self.apply_one("grid-template-columns", cols);
                    }
                    (_, true) => {
                        self.grid_auto_flow = Some(if cols.contains("dense") {
                            AutoFlow::ColDense
                        } else {
                            AutoFlow::Col
                        });
                        self.apply_one("grid-template-rows", rows);
                        self.apply_one("grid-auto-columns", strip_auto_flow(cols));
                    }
                    _ => {
                        self.apply_one("grid-template-rows", rows);
                        if !cols.is_empty() {
                            self.apply_one("grid-template-columns", cols);
                        }
                    }
                }
            }
            // Значение бывает составным: `row fill-reverse`, `column
            // track-reverse`. Сверка со строкой ЦЕЛИКОМ путала ось на каждом
            // таком тесте.
            "grid-lanes-pack" => self.lanes_dense = v.contains("dense"),
            // Порог «равенства» лунок при авто-выборе (css-grid-3): лунки с
            // разницей заполнения меньше порога считаются равными и берутся в
            // ПОРЯДКЕ ДОКУМЕНТА. `normal` (дефолт!) = 1em, `infinite` —
            // строгий порядок укладки безотносительно высот.
            "flow-tolerance" | "item-tolerance" => {
                self.lanes_tolerance = match v.trim() {
                    "normal" => None,
                    "infinite" => Some(Len::Px(f32::INFINITY)),
                    t => Len::parse(t),
                };
            }
            // Первая часть — ось лунок, дальше — реверсы: `fill-reverse`
            // заполняет лунки с другого конца, `track-reverse` перечисляет
            // сами лунки в обратном порядке (css-grid-3).
            "grid-lanes-direction" => {
                self.lanes_row = Some(v.split_whitespace().next() == Some("row"));
                self.lanes_fill_reverse = v.split_whitespace().any(|w| w == "fill-reverse");
                self.lanes_track_reverse = v.split_whitespace().any(|w| w == "track-reverse");
            }
            "grid-gap" => self.apply_one("gap", v),
            "grid-row-gap" => self.apply_one("row-gap", v),
            "grid-column-gap" => self.apply_one("column-gap", v),
            _ => self.apply_grid_placement(key, val, v, hit),
        }
    }
}
