//! Computed::apply_one: grid-*, grid lanes.

use crate::style::computed::*;
use crate::style::values::value::Len;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_grid(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // `repeat(auto-fill | auto-fit, minmax(N, 1fr))` — «сколько
            // влезет»: число колонок известно только раскладке. Раньше запись
            // не разбиралась вовсе, и вся сетка схлопывалась в одну колонку.
            //
            // СДЕЛАНО (прежний откат снят): списку СЛОЖНЕЕ одинокого повтора
            // (`10px repeat(auto-fill, 30px) 50px`) пишется и `grid_tracks`,
            // и `grid_cols`, а сам повтор внутри непустого списка
            // разворачивает раскладка лунок. Замерено по css-grid (1133 пары)
            // 645 -> 648 и по всему css3 2417 -> 2420: приобретено 3
            // (`grid-auto-repeat-multiple-values-002/003`,
            // `row-auto-repeat-013`), потеряно 0.
            //
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: писать список ВСЕГДА, в том числе для
            // одинокого повтора. По css-grid 645 -> 604: приобретено 6,
            // потеряно 47 (`column-auto-repeat-001/013/017/018/027..030`,
            // `-auto-001/011..014/025/026`, `-fit-content-004/005`,
            // `-max-content-001/002` и далее). Записанный список уводит
            // одинокий повтор с прежнего пути раскладки, а тот считает число
            // повторов точнее: по долям, по содержимому и по `fit-content`.
            // Отсюда условие `l.len() > 1` ниже — оно не заплатка, а граница
            // между двумя честными путями счёта повторов.
            "grid-template-columns" if v.contains("auto-fill") || v.contains("auto-fit") => {
                // `subgrid [a] repeat(auto-fill, [b])` — повтор СПИСКА ИМЁН
                // подсетки (css-grid-2 §subgrid-listing, `<line-name-list>`),
                // а не дорожек: такой элемент — подсетка, и дорожки ей выдаёт
                // родитель (`taffy::compute::grid::subgrid`). Прежде запись
                // уходила в разбор авто-повтора, и элемент подсеткой не был
                // (`subgrid/repeat-auto-fill-005`).
                self.grid_col_line_names = parse_line_names(v);
                if v.trim_start().starts_with("subgrid") {
                    self.subgrid_cols = true;
                    self.subgrid = true;
                    self.grid_cols = count_tracks(v);
                    self.grid_tracks = parse_tracks(v);
                    return;
                }
                // css-grid-1 `<auto-track-list>`: ВОКРУГ авто-повтора допустим
                // только `<fixed-size>`. css-grid-3 §7.2.1 ослабила запись
                // ВНУТРИ `repeat()`, снаружи всё по-прежнему — голая
                // интрин-дорожка делает объявление негодным, и оно целиком
                // падает в `none`. Сами тесты пишут это комментарием: «This is
                // not currently a valid track definition and will fall back to
                // none». Перепись корпуса
                // (`target/scout-lanes-9e-invalid.txt`): таких объявлений 14,
                // шесть из них — законные `minmax(…)`, которых правило не
                // касается; остаются ровно восемь целевых файлов.
                if auto_repeat_outside_intrinsic(v) {
                    return;
                }
                self.grid_auto_fill_min = auto_fill_min(v);
                self.grid_auto_fill_tracks = auto_fill_tracks(v);
                self.auto_repeat_body_cols = auto_fill_body_tracks(v);
                // Список пишется и при авто-повторе: дорожки ДО и ПОСЛЕ него
                // (`max-content repeat(auto-fill, max-content) max-content`)
                // иначе теряются целиком. Разворот самого повтора при
                // непустом списке делает раскладка лунок — это и есть
                // условие возврата из прежнего отката.
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v143, `scout-lanes-2026-09e.md`
            // патч II): писать одинокий `AutoRepeat` с телом > 1 дорожки
            // списком. Срез css-viewport+css-transforms+css-grid+css-position+
            // CSS2 8273: +3 при −6 — `column/row-auto-repeat-auto-011` (0.00 →
            // 7.78), `column-auto-repeat-fit-content-004` (→ 34.01),
            // `-max-content-004` (→ 33.54), `column/row-auto-repeat-minmax-005`
            // (→ 7.78). Синтаксической границы между целями и заложниками нет
            // (`auto 50px` красен в колонках и зелен в рядах) — нужен разбор
            // по контексту, а не по форме тела.
                if let Some(list) = parse_tracks(v).filter(|l| l.len() > 1) {
                    self.grid_cols = count_tracks(v);
                    self.grid_tracks = Some(list);
                }
                let (max_auto, max_fr) = auto_fill_max(v);
                self.auto_repeat_cols = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_min,
                    body: auto_fill_body(v),
                    track_pct: auto_fill_pct(v),
                    intrinsic: auto_fill_intrinsic(v),
                    intrinsic_min: auto_fill_intrinsic(v) && v.contains("min-content"),
                    fit_px: auto_fill_fit_px(v),
                    max_auto,
                    max_fr,
                });
            }
            // То же по РЯДАМ: у раскладки лунками дорожки задают ряды, когда
            // `grid-lanes-direction: row` (`row-auto-repeat-001`).
            "grid-template-rows" if v.contains("auto-fill") || v.contains("auto-fit") => {
                // Подсетка со списком имён в повторе — см. колонки выше.
                self.grid_row_line_names = parse_line_names(v);
                if v.trim_start().starts_with("subgrid") {
                    self.subgrid_rows = true;
                    self.subgrid = true;
                    self.grid_rows = parse_tracks(v);
                    return;
                }
                // Та же негодность по РЯДАМ (`row-auto-repeat-auto-005`,
                // `row-auto-repeat-{fit,max,min}-content-003`).
                if auto_repeat_outside_intrinsic(v) {
                    return;
                }
                self.grid_auto_fill_row = auto_fill_min(v);
                self.auto_repeat_body_rows = auto_fill_body_tracks(v);
                // Только когда вокруг повтора ЕСТЬ свои дорожки: одинокий
                // повтор целиком ведёт прежний путь раскладки, он считает
                // число повторов точнее (доли, содержимое, `fit-content`).
                if let Some(list) = parse_tracks(v).filter(|l| l.len() > 1) {
                    self.grid_rows = Some(list);
                }
                let (max_auto, max_fr) = auto_fill_max(v);
                self.auto_repeat_rows = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_row,
                    body: auto_fill_body(v),
                    track_pct: auto_fill_pct(v),
                    intrinsic: auto_fill_intrinsic(v),
                    intrinsic_min: auto_fill_intrinsic(v) && v.contains("min-content"),
                    fit_px: auto_fill_fit_px(v),
                    max_auto,
                    max_fr,
                });
            }
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
                self.grid_auto_cols_list = if all.len() > 1 { all.clone() } else { Vec::new() };
                self.grid_auto_cols = all.into_iter().next();
            }
            "grid-auto-rows" => {
                let all = parse_tracks(v).unwrap_or_default();
                self.grid_auto_rows_list = if all.len() > 1 { all.clone() } else { Vec::new() };
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
                    Some(gpui::GridNamedLine::Line(name, 0)) => Some(gpui::GridNamedLine::Line(name.clone(), 0)),
                    _ => None,
                };
                let row_start = named(0);
                let col_start = if parts.len() > 1 { named(1) } else { ident(&row_start) };
                let row_end = if parts.len() > 2 { named(2) } else { ident(&row_start) };
                let col_end = if parts.len() > 3 { named(3) } else { ident(&col_start) };
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
