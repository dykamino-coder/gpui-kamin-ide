//! Computed::apply_one: grid-*, grid lanes.

use crate::style::computed::*;
use crate::style::values::value::Len;

mod placement;
mod templates;

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
            _ => self.apply_grid_templates(key, val, v, hit),
        }
    }
}
