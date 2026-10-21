//! Группа свойств `Computed::apply_one`: `grid-*` и раскладка лунками (grid
//! lanes). Здесь — шаблоны дорожек с авто-повтором `repeat(auto-fill |
//! auto-fit, …)` (css-grid-1 `<auto-track-list>`, подсетка с повтором имён
//! по css-grid-2 §subgrid-listing) в `AutoRepeat`; остальные шаблоны и
//! размещение — в подмодулях `templates`, `placement`.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::Len;

mod placement;
mod templates;

impl Computed {
    pub(crate) fn apply_grid(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        match key {
            // `repeat(auto-fill | auto-fit, minmax(N, 1fr))` — «сколько
            // влезет»: число колонок известно только раскладке. Раньше запись
            // не разбиралась вовсе, и вся сетка схлопывалась в одну колонку.
            //
            // Списку СЛОЖНЕЕ одинокого повтора (`10px repeat(auto-fill, 30px)
            // 50px`) пишется и `grid_tracks`, и `grid_cols`, а сам повтор
            // внутри непустого списка разворачивает раскладка лунок
            // (`grid-auto-repeat-multiple-values-002/003`,
            // `row-auto-repeat-013`).
            //
            // Список для ОДИНОКОГО повтора не писать: он уводит повтор с
            // прежнего пути раскладки, а тот считает число повторов точнее —
            // по долям, по содержимому и по `fit-content`
            // (`column-auto-repeat-001/013/017/018/027..030`,
            // `-auto-001/011..014/025/026`, `-fit-content-004/005`,
            // `-max-content-001/002`). Отсюда условие `l.len() > 1` ниже —
            // оно не заплатка, а граница между двумя путями счёта повторов.
            "grid-template-columns" if v.contains("auto-fill") || v.contains("auto-fit") => {
                // `subgrid [a] repeat(auto-fill, [b])` — повтор СПИСКА ИМЁН
                // подсетки (css-grid-2 §subgrid-listing, `<line-name-list>`),
                // а не дорожек: такой элемент — подсетка, и дорожки ей выдаёт
                // родитель (`taffy::compute::grid::subgrid`). Прежде запись
                // уходила в разбор авто-повтора, и элемент подсеткой не был
                // (`subgrid/repeat-auto-fill-005`).
                self.grid_col_line_names = parse_line_names(v);
                if v.trim_start().starts_with("subgrid") {
                    self.accept_declaration();
                    self.subgrid_cols = true;
                    self.subgrid = true;
                    self.grid_cols = count_tracks(v);
                    self.grid_tracks = parse_tracks(v, vcx);
                    return;
                }
                // css-grid-1 `<auto-track-list>`: ВОКРУГ авто-повтора допустим
                // только `<fixed-size>`. css-grid-3 §7.2.1 ослабила запись
                // ВНУТРИ `repeat()`, снаружи всё по-прежнему — голая
                // интрин-дорожка делает объявление негодным, и оно целиком
                // падает в `none`. Сами тесты пишут это комментарием: «This is
                // not currently a valid track definition and will fall back to
                // none». Перепись корпуса:
                // таких объявлений 14,
                // шесть из них — законные `minmax(…)`, которых правило не
                // касается; остаются ровно восемь целевых файлов.
                if auto_repeat_outside_intrinsic(v) {
                    return;
                }
                self.grid_auto_fill_min = auto_fill_min(v, vcx);
                self.grid_auto_fill_tracks = auto_fill_tracks(v, vcx);
                self.auto_repeat_body_cols = auto_fill_body_tracks(v, vcx);
                // Список пишется и при авто-повторе: дорожки ДО и ПОСЛЕ него
                // (`max-content repeat(auto-fill, max-content) max-content`)
                // иначе теряются целиком. Разворот самого повтора при
                // непустом списке делает раскладка лунок — это и есть
                // условие возврата из прежнего отката.
                // Одинокий `AutoRepeat` с телом > 1 дорожки списком не писать:
                // ломаются `column/row-auto-repeat-auto-011`,
                // `column-auto-repeat-fit-content-004`, `-max-content-004`,
                // `column/row-auto-repeat-minmax-005`. Синтаксической границы
                // между целями и заложниками нет (`auto 50px` красен в колонках
                // и зелен в рядах) — нужен разбор по контексту, а не по форме
                // тела.
                if let Some(list) = parse_tracks(v, vcx).filter(|l| l.len() > 1) {
                    self.grid_cols = count_tracks(v);
                    self.grid_tracks = Some(list);
                }
                let (max_auto, max_fr) = auto_fill_max(v);
                self.auto_repeat_cols = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_min,
                    body: auto_fill_body(v, vcx),
                    track_pct: auto_fill_pct(v, vcx),
                    intrinsic: auto_fill_intrinsic(v, vcx),
                    intrinsic_min: auto_fill_intrinsic(v, vcx) && v.contains("min-content"),
                    fit_px: auto_fill_fit_px(v, vcx),
                    max_auto,
                    max_fr,
                });
                self.accept_declaration();
            }
            // То же по РЯДАМ: у раскладки лунками дорожки задают ряды, когда
            // `grid-lanes-direction: row` (`row-auto-repeat-001`).
            "grid-template-rows" if v.contains("auto-fill") || v.contains("auto-fit") => {
                // Подсетка со списком имён в повторе — см. колонки выше.
                self.grid_row_line_names = parse_line_names(v);
                if v.trim_start().starts_with("subgrid") {
                    self.accept_declaration();
                    self.subgrid_rows = true;
                    self.subgrid = true;
                    self.grid_rows = parse_tracks(v, vcx);
                    return;
                }
                // Та же негодность по РЯДАМ (`row-auto-repeat-auto-005`,
                // `row-auto-repeat-{fit,max,min}-content-003`).
                if auto_repeat_outside_intrinsic(v) {
                    return;
                }
                self.grid_auto_fill_row = auto_fill_min(v, vcx);
                self.auto_repeat_body_rows = auto_fill_body_tracks(v, vcx);
                // Только когда вокруг повтора ЕСТЬ свои дорожки: одинокий
                // повтор целиком ведёт прежний путь раскладки, он считает
                // число повторов точнее (доли, содержимое, `fit-content`).
                if let Some(list) = parse_tracks(v, vcx).filter(|l| l.len() > 1) {
                    self.grid_rows = Some(list);
                }
                let (max_auto, max_fr) = auto_fill_max(v);
                self.auto_repeat_rows = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_row,
                    body: auto_fill_body(v, vcx),
                    track_pct: auto_fill_pct(v, vcx),
                    intrinsic: auto_fill_intrinsic(v, vcx),
                    intrinsic_min: auto_fill_intrinsic(v, vcx) && v.contains("min-content"),
                    fit_px: auto_fill_fit_px(v, vcx),
                    max_auto,
                    max_fr,
                });
                self.accept_declaration();
            }
            _ => self.apply_grid_templates(key, v, hit, vcx),
        }
    }
}
