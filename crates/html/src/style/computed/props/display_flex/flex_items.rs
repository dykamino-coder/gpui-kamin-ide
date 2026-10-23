//! Computed::apply_display_flex, продолжение цепочки: flex-wrap, flex-grow/-shrink, flex-basis,
//! align-self/align-items. Ветви в исходном порядке; не совпавший ключ уходит в apply_gap_place.

use super::values;
use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_flex_items(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "flex-wrap" => {
                if let Some(w) = values::flex_wrap(v) {
                    self.flex_wrap = accept_parsed!(self, Some(w.wrap));
                    // Обратный перенос кладёт строки с другого края: одна строка
                    // в контейнере уезжает вниз, а не остаётся вверху.
                    self.flex_wrap_reverse = Some(w.reverse);
                    self.flex_balance = Some(w.balance);
                }
            }
            "flex-line-count" => {
                // css-flexbox-2 §5.3: `<integer [1,∞]>`; действует только у
                // balance (как в Blink — `balance-min-line-count-007/008`).
                if let Ok(n) = v.trim().parse::<u32>()
                    && n >= 1
                {
                    self.flex_line_count =
                        accept_parsed!(self, Some(n.min(u32::from(u16::MAX)) as u16));
                }
            }
            // Отрицательные значения невалидны (css-flexbox-1 §7.2: «Negative
            // values are not allowed») — объявление отбрасывается целиком
            // (`flex-shrink-002`, `flex-basis-004`).
            "flex-grow" => {
                if let Some(g) = flex_factor(v, vcx) {
                    self.flex_grow = accept_parsed!(self, Some(g));
                }
            }
            "flex-shrink" => {
                if let Some(g) = flex_factor(v, vcx) {
                    self.flex_shrink = accept_parsed!(self, Some(g));
                }
            }
            "flex-basis" => {
                if let Some((basis, content)) = values::flex_basis(v, vcx) {
                    self.flex_basis = accept_parsed!(self, Some(basis));
                    self.basis_content = Some(content);
                }
            }
            "align-self" if v.trim() == "inherit" => {
                self.align_self_inherit = true;
            }
            "align-items" | "justify-items" | "align-content" | "justify-content"
            | "justify-self"
                if v.trim() == "inherit" =>
            {
                self.align_inherit |= match key {
                    "align-items" => ainh::ALIGN_ITEMS,
                    "justify-items" => ainh::JUSTIFY_ITEMS,
                    "align-content" => ainh::ALIGN_CONTENT,
                    "justify-content" => ainh::JUSTIFY_CONTENT,
                    _ => ainh::JUSTIFY_SELF,
                };
            }
            "align-self" => {
                // `left`/`right` у `align-self` недействительны: это
                // `<self-position>` без них, физические стороны есть только у
                // `justify-self` (css-align-3 §6.1) — объявление отбрасывается
                // (`align-self-static-position-008`: `right` ждёт `start`;
                // `grid-abspos-staticpos-align-self-rtl-*`).
                let last = v.split_whitespace().last();
                if !matches!(last, Some("left") | Some("right"))
                    && let Ok(a) = align_keyword(v)
                {
                    self.align_self = accept_parsed!(self, a);
                    self.align_self_decl = Some(a);
                    self.align_self_inherit = false;
                    self.align_self_safe = is_safe(v);
                    self.align_self_normal = v.trim() == "normal";
                    self.align_self_own_axis =
                        matches!(last, Some("self-start") | Some("self-end"));
                    self.align_self_flex_kw = matches!(last, Some("flex-start") | Some("flex-end"));
                    self.align_self_last = v.split_whitespace().any(|w| w == "last");
                }
            }
            "align-items" => {
                if let Ok(a) = align_keyword(v) {
                    self.align_inherit &= !ainh::ALIGN_ITEMS;
                    self.align_items = accept_parsed!(self, a);
                    self.align_items_safe = is_safe(v);
                    self.align_items_last = v.split_whitespace().any(|w| w == "last");
                }
            }
            // `space-evenly` и `space-around` различаются шириной крайних
            // промежутков — сведение их в одно значение расходилось с
            // браузером на 27 точек (поймано сравнением).
            "justify-content" => {
                let parsed = parsed_value!(
                    self,
                    parse_justify(v)
                        .map(Some)
                        .or_else(|| (v == "normal").then_some(None))
                );
                self.align_inherit &= !ainh::JUSTIFY_CONTENT;
                self.justify_content = parsed;
                if v == "normal" {
                    self.accept_declaration();
                }
                self.justify_content_safe = is_safe(v);
            }
            _ => self.apply_gap_place(key, v, hit, vcx),
        }
    }
}
