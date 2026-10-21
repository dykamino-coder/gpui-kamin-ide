//! Computed::apply_display_flex, продолжение цепочки: flex-wrap, flex-*, flex-basis,
//! align-self/align-items. Ветви в исходном порядке; не совпавший ключ уходит в apply_gap_place.

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
                // css-flexbox-2 §5.2: `nowrap | [ wrap | wrap-reverse ] || balance`;
                // `balance` без `wrap*` ведёт себя как `wrap`. Невалидное
                // сочетание (`nowrap balance`, два режима) отбрасывается.
                let (mut wrap, mut reverse, mut balance, mut nowrap, mut modes, mut valid) =
                    (false, false, 0u8, false, 0u8, true);
                for word in v.split_ascii_whitespace() {
                    match word {
                        "wrap" => modes += 1,
                        "wrap-reverse" => {
                            modes += 1;
                            reverse = true;
                        }
                        "balance" => balance += 1,
                        "nowrap" => nowrap = true,
                        _ => valid = false,
                    }
                }
                wrap |= modes > 0 || balance > 0;
                if !v.is_empty()
                    && v.split_whitespace().filter(|w| *w == "nowrap").count() <= 1
                    && valid
                    && modes <= 1
                    && balance <= 1
                    && (!nowrap || (modes == 0 && balance == 0))
                {
                    self.flex_wrap = accept_parsed!(self, Some(wrap));
                    // Обратный перенос кладёт строки с другого края: одна строка
                    // в контейнере уезжает вниз, а не остаётся вверху.
                    self.flex_wrap_reverse = Some(reverse);
                    self.flex_balance = Some(balance > 0);
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
            // `flex: 1` — сокращение для grow/shrink/basis; берём первое число.
            // `flex: <рост> <сжатие> <основа>` со всеми сокращёнными формами.
            // Раньше бралось только первое число, и `flex: 0 0 200px` терял
            // фиксированную основу — блок начинал растягиваться.
            "flex" => {
                let (grow, shrink, basis, content) = parsed_value!(self, flex_value(v, vcx));
                self.flex_grow = Some(grow);
                self.flex_shrink = Some(shrink);
                self.flex_basis = Some(basis);
                self.basis_content = Some(content);
            }
            "flex-basis" if v == "content" => {
                self.flex_basis = accept_parsed!(self, Some(Len::Auto));
                self.basis_content = Some(true);
            }
            "flex-basis" => {
                if let Some(l) = Len::parse(v, vcx)
                    && !matches!(l, Len::Px(x) | Len::Pct(x) if x < 0.0)
                {
                    self.flex_basis = accept_parsed!(self, Some(l));
                    self.basis_content = Some(false);
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

fn flex_value(v: &str, vcx: ValueCx<'_>) -> Option<(f32, f32, Len, bool)> {
    let basis = |v: &str| {
        if v == "content" {
            Some((Len::Auto, true))
        } else {
            if crate::style::values::value::number(v, vcx).is_some_and(|n| n != 0.0) {
                return None;
            }
            size_range::parse_nonnegative_size(v, vcx).map(|len| (len, false))
        }
    };
    let tokens = split_outside_parens(v);
    let (g, s, (b, content)) = match tokens.as_slice() {
        [one] if one == "auto" => (1.0, 1.0, (Len::Auto, false)),
        [one] if one == "initial" => (0.0, 1.0, (Len::Auto, false)),
        [one] if one == "none" => (0.0, 0.0, (Len::Auto, false)),
        [one] => match flex_factor(one, vcx) {
            Some(g) => (g, 1.0, (Len::Pct(0.0), false)),
            None => (1.0, 1.0, basis(one)?),
        },
        [a, b] => {
            let g = flex_factor(a, vcx)?;
            match flex_factor(b, vcx) {
                Some(s) => (g, s, (Len::Pct(0.0), false)),
                None => (g, 1.0, basis(b)?),
            }
        }
        [a, b, c] => (flex_factor(a, vcx)?, flex_factor(b, vcx)?, basis(c)?),
        _ => return None,
    };
    Some((g, s, b, content))
}
