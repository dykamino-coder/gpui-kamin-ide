//! Computed::apply_one: column*, break-*, *-rule*, margin-trim, page, orphans/widows.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod gap_rules;
mod rule_props;
use gap_rules::{gap_color, gap_inset, gap_list, gap_style, gap_width};

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_multicol(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // css-break-3 §4.4: `<integer [1,∞]>`; ноль и отрицательное
            // невалидны — объявление отбрасывается.
            "orphans" | "widows" => {
                if let Ok(n) = v.parse::<u16>()
                    && n >= 1
                {
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
                    self.column_count = None;
                } else if let Ok(n) = v.trim().parse::<u16>()
                    && n > 0
                {
                    self.column_count = Some(n);
                }
            }
            "column-width" => {
                if v.trim() == "auto" {
                    self.column_width = None;
                } else if let Some(l) = Len::parse(v.trim()) {
                    // Отрицательная и нулевая ширина колонки невалидны.
                    if !matches!(l, Len::Px(w) if w <= 0.0) {
                        self.column_width = Some(l);
                    }
                }
            }
            // `column-height: auto | <length [0,∞]>` (css-multicol-2 §ch):
            // отрицательная невалидна и не затирает прежнее; ноль — законная
            // высота (`columns: 2 / 0`, `column-height-021…023`).
            "column-height" => {
                if v.trim() == "auto" {
                    self.column_height = None;
                } else if let Some(l) = Len::parse(v.trim())
                    && !matches!(l, Len::Px(h) if h < 0.0)
                {
                    self.column_height = Some(l);
                }
            }
            // `column-wrap: auto | nowrap | wrap` (css-multicol-2 §cwr);
            // `auto` — отсутствие значения, решается в укладке по
            // `column-height`.
            "column-wrap" => {
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
                self.page =
                    (!t.is_empty() && !t.eq_ignore_ascii_case("auto")).then(|| t.to_string());
            }
            // `column-fill`: балансировать ли колонки (дефолт balance).
            "column-fill" => self.column_fill_auto = Some(v.trim() == "auto"),
            // `scroll-marker-group: none | [before|after] || [links|tabs]`
            // (css-overflow-5): нужна только сторона, `links`/`tabs` — роль.
            "scroll-marker-group" => {
                let words: Vec<&str> = v.split_whitespace().collect();
                self.scroll_marker_group = if words.contains(&"before") {
                    Some(true)
                } else if words.contains(&"after") {
                    Some(false)
                } else {
                    None
                };
            }
            // `column-span: all` — растяжка на все колонки.
            "column-span" => self.column_span = Some(v.trim() == "all"),
            // `avoid`, `avoid-column`, `avoid-page`, `avoid-region` — все
            // запрещают разрыв ВНУТРИ коробки; `auto` разрешает.
            // `page-break-inside` — устаревшее написание того же (css-break-3
            // §6.4 требует считать их одним свойством).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v153, `scout-boxdeco-2026-09.md`):
            // `box-decoration-break: clone` — украшение на каждом фрагменте
            // (21 хунк: разбор, `Kid::clone_dec`, ветка в `fill_at`, `frags_of`,
            // `clone_fragment`). Срез `L-brk` 2874: +2/−2 при ожидании +6…+19 —
            // `clone-004`, `-012` взяты, но `clone-005.tentative` 0.00 → 99.00 и
            // `clone-007` 0.00 → 2.08. Ветка `clone` в `fill_at` ломает уже
            // работавший `slice` у вложенных случаев; нужен отдельный проход
            // планирования фрагментов, а не правка общей укладки.
            "break-inside" | "page-break-inside" => {
                self.break_inside_avoid = v.trim().starts_with("avoid")
            }
            // Префиксное написание — то же свойство (`border-image-000`,
            // `-webkit-box-decoration-break: clone`).
            "box-decoration-break" | "-webkit-box-decoration-break" => {
                self.bdb_clone = v.trim().eq_ignore_ascii_case("clone")
            }
            "break-before" | "page-break-before" => {
                self.break_before_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
                // css-break-4 §3.1 «avoid break values». Тип фрагментации не
                // различаем — ровно как уже написанный `break_inside_avoid`
                // (`starts_with("avoid")`). Blink здесь строже
                // (`fragmentation_utils.cc:108-121 IsAvoidBreakValue`:
                // `avoid-page` в колонках не действует); упрощение осознанное
                // и безопасное, пока страницы вне зоны патча (§2 отчёта).
                self.break_before_avoid = v.trim().starts_with("avoid");
            }
            "break-after" | "page-break-after" => {
                self.break_after_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
                self.break_after_avoid = v.trim().starts_with("avoid");
            }
            _ => self.apply_multicol_rules(key, val, v, hit),
        }
    }
}
