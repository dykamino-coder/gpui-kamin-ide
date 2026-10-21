//! Группа свойств `Computed::apply_one`: многоколоночность и фрагментация —
//! `column-*` (css-multicol-1/2), `break-*`/`page-break-*` и
//! `box-decoration-break` (css-break-3/4), `orphans`/`widows`, `page`,
//! `scroll-marker-group`/`scroll-target-group`; линейки `*-rule*`, щели и
//! `margin-trim` — в подмодулях `gap_rules`, `rule_props`.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod gap_rules;
mod rule_props;
use gap_rules::{gap_color, gap_inset, gap_list, gap_style, gap_width};

impl Computed {
    pub(crate) fn apply_multicol(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        match key {
            // css-break-3 §4.4: `<integer [1,∞]>`; ноль и отрицательное
            // невалидны — объявление отбрасывается.
            "orphans" | "widows" => {
                if let Ok(n) = v.parse::<u16>()
                    && n >= 1
                {
                    self.accept_declaration();
                    if key == "orphans" {
                        self.orphans = Some(n);
                    } else {
                        self.widows = Some(n);
                    }
                }
            }

            // --- Многоколоночный поток ----------------------------------------
            // Невалидное значение НЕ затирает прежнее (каскад CSS
            // отбрасывает объявление целиком): `column-count: -1` после
            // `column-count: 2` оставляет двойку. Ноль и минус невалидны.
            "column-count" => {
                if v.trim() == "auto" {
                    self.column_count = accept_parsed!(self, None);
                } else if let Ok(n) = v.trim().parse::<u16>()
                    && n > 0
                {
                    self.column_count = accept_parsed!(self, Some(n));
                }
            }
            "column-width" => {
                if v.trim() == "auto" {
                    self.column_width = accept_parsed!(self, None);
                } else if let Some(l) = Len::parse(v.trim(), vcx) {
                    // Отрицательная и нулевая ширина колонки невалидны.
                    if !matches!(l, Len::Px(w) if w <= 0.0) {
                        self.column_width = accept_parsed!(self, Some(l));
                    }
                }
            }
            // `column-height: auto | <length [0,∞]>` (css-multicol-2 §ch):
            // отрицательная невалидна и не затирает прежнее; ноль — законная
            // высота (`columns: 2 / 0`, `column-height-021…023`).
            "column-height" => {
                if v.trim() == "auto" {
                    self.column_height = accept_parsed!(self, None);
                } else if let Some(l) = Len::parse(v.trim(), vcx)
                    && !matches!(l, Len::Px(h) if h < 0.0)
                {
                    self.column_height = accept_parsed!(self, Some(l));
                }
            }
            // `column-wrap: auto | nowrap | wrap` (css-multicol-2 §cwr);
            // `auto` — отсутствие значения, решается в укладке по
            // `column-height`.
            "column-wrap" => {
                if matches!(v.trim(), "auto" | "wrap" | "nowrap") {
                    self.accept_declaration();
                } else {
                    return;
                }
                self.column_wrap = match v.trim() {
                    "wrap" => Some(true),
                    "nowrap" => Some(false),
                    _ => None,
                };
            }
            // `page: auto | <custom-ident>` (css-page-3 §"Using named pages").
            // Имя регистрозависимо; `auto` — ключевое слово без регистра и
            // хранится отсутствием значения.
            "page" => {
                let t = v.trim();
                if !crate::style::css::selector_tokens::ident(t) {
                    return;
                }
                self.accept_declaration();
                self.page =
                    (!t.is_empty() && !t.eq_ignore_ascii_case("auto")).then(|| t.to_string());
            }
            // `column-fill`: балансировать ли колонки (дефолт balance).
            "column-fill" => {
                self.column_fill_auto = apply_parsed!(
                    self,
                    match v {
                        "auto" => Some(true),
                        "balance" | "balance-all" => Some(false),
                        _ => None,
                    }
                )
            }
            // `scroll-marker-group: none | [before|after] || [links|tabs]`
            // (css-overflow-5): нужна только сторона, `links`/`tabs` — роль.
            "scroll-target-group" => {
                if !matches!(v, "auto" | "none") {
                    return;
                }
                self.scroll_target_group =
                    accept_parsed!(self, Some(v.trim().eq_ignore_ascii_case("auto")));
            }
            "scroll-marker-group" => {
                let words: Vec<_> = v.split_whitespace().collect();
                if words.is_empty()
                    || words
                        .iter()
                        .any(|w| !matches!(*w, "before" | "after" | "links" | "tabs" | "none"))
                {
                    return;
                }
                self.scroll_marker_group = parsed_value!(
                    self,
                    if v == "none" {
                        Some(None)
                    } else if words.contains(&"before") {
                        Some(Some(true))
                    } else if words.contains(&"after") {
                        Some(Some(false))
                    } else {
                        None
                    }
                );
            }

            // `column-span: all` — растяжка на все колонки.
            "column-span" => {
                self.column_span = apply_parsed!(
                    self,
                    match v {
                        "all" => Some(true),
                        "none" => Some(false),
                        _ => None,
                    }
                )
            }
            // `avoid`, `avoid-column`, `avoid-page`, `avoid-region` — все
            // запрещают разрыв ВНУТРИ коробки; `auto` разрешает.
            // `page-break-inside` — устаревшее написание того же (css-break-3
            // §6.4 требует считать их одним свойством).
            "break-inside" | "page-break-inside" => {
                if !matches!(
                    v,
                    "auto" | "avoid" | "avoid-column" | "avoid-page" | "avoid-region"
                ) {
                    return;
                }
                self.break_inside_avoid = accept_parsed!(self, v.trim().starts_with("avoid"))
            }
            // Префиксное написание — то же свойство (`border-image-000`,
            // `-webkit-box-decoration-break: clone`).
            "box-decoration-break" | "-webkit-box-decoration-break" => {
                if !matches!(v, "slice" | "clone") {
                    return;
                }
                self.bdb_clone = accept_parsed!(self, v.trim().eq_ignore_ascii_case("clone"))
            }
            "break-before" | "page-break-before" => {
                if !matches!(
                    v,
                    "auto"
                        | "avoid"
                        | "avoid-column"
                        | "avoid-page"
                        | "avoid-region"
                        | "column"
                        | "page"
                        | "always"
                        | "left"
                        | "right"
                        | "recto"
                        | "verso"
                        | "region"
                        | "all"
                ) {
                    return;
                }
                self.break_before_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
                // css-break-4 §3.1 «avoid break values». Тип фрагментации не
                // различаем — ровно как уже написанный `break_inside_avoid`
                // (`starts_with("avoid")`). Blink здесь строже
                // (`fragmentation_utils.cc:108-121 IsAvoidBreakValue`:
                // `avoid-page` в колонках не действует); упрощение осознанное
                // и безопасное, пока страницы не раскладываются.
                self.break_before_avoid = v.trim().starts_with("avoid");
                self.accept_declaration();
            }
            "break-after" | "page-break-after" => {
                if !matches!(
                    v,
                    "auto"
                        | "avoid"
                        | "avoid-column"
                        | "avoid-page"
                        | "avoid-region"
                        | "column"
                        | "page"
                        | "always"
                        | "left"
                        | "right"
                        | "recto"
                        | "verso"
                        | "region"
                        | "all"
                ) {
                    return;
                }
                self.break_after_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
                self.break_after_avoid = v.trim().starts_with("avoid");
                self.accept_declaration();
            }
            _ => self.apply_multicol_rules(key, v, hit, vcx),
        }
    }
}
