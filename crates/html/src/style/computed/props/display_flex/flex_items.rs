//! Computed::apply_display_flex, продолжение цепочки: flex-wrap, flex-*, flex-basis, align-self/align-items. Ветви в исходном порядке; не совпавший ключ уходит в apply_gap_place.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_flex_items(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
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
                if valid && modes <= 1 && balance <= 1 && (!nowrap || (modes == 0 && balance == 0))
                {
                    self.flex_wrap = Some(wrap);
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
                    self.flex_line_count = Some(n.min(u32::from(u16::MAX)) as u16);
                }
            }
            // Отрицательные значения невалидны (css-flexbox-1 §7.2: «Negative
            // values are not allowed») — объявление отбрасывается целиком
            // (`flex-shrink-002`, `flex-basis-004`).
            "flex-grow" => {
                if let Some(g) = flex_factor(v) {
                    self.flex_grow = Some(g);
                }
            }
            "flex-shrink" => {
                if let Some(g) = flex_factor(v) {
                    self.flex_shrink = Some(g);
                }
            }
            // `flex: 1` — сокращение для grow/shrink/basis; берём первое число.
            // `flex: <рост> <сжатие> <основа>` со всеми сокращёнными формами.
            // Раньше бралось только первое число, и `flex: 0 0 200px` терял
            // фиксированную основу — блок начинал растягиваться.
            "flex" => match v {
                "auto" => {
                    self.flex_grow = Some(1.0);
                    self.flex_shrink = Some(1.0);
                    self.flex_basis = Some(Len::Auto);
                }
                "none" => {
                    self.flex_grow = Some(0.0);
                    self.flex_shrink = Some(0.0);
                    self.flex_basis = Some(Len::Auto);
                }
                "initial" => {
                    self.flex_grow = Some(0.0);
                    self.flex_shrink = Some(1.0);
                    self.flex_basis = Some(Len::Auto);
                }
                _ => {
                    // Опущенные части сокращения берут НЕ начальные значения
                    // свойств: рост и сжатие становятся 1, а основа — 0%, а не
                    // `auto`. Отсюда весь смысл записи `flex: 1`: элемент
                    // делит место поровну, забыв свою ширину. Раньше основа при
                    // двух числах оставалась `auto`, и `flex: 0 1` держал
                    // ширину элемента вместо нуля.
                    let parts: Vec<&str> = v.split_whitespace().collect();
                    let number = |t: &str| flex_factor(t);
                    match parts.as_slice() {
                        [one] => match number(one) {
                            Some(g) => {
                                self.flex_grow = Some(g);
                                self.flex_shrink = Some(1.0);
                                self.flex_basis = Some(Len::Pct(0.0));
                            }
                            None => {
                                self.flex_grow = Some(1.0);
                                self.flex_shrink = Some(1.0);
                                self.flex_basis = Len::parse(one);
                            }
                        },
                        [a, b] => {
                            self.flex_grow = number(a);
                            match number(b) {
                                Some(shrink) => {
                                    self.flex_shrink = Some(shrink);
                                    self.flex_basis = Some(Len::Pct(0.0));
                                }
                                None => {
                                    self.flex_shrink = Some(1.0);
                                    self.flex_basis = Len::parse(b);
                                }
                            }
                        }
                        [a, b, c] => {
                            // Безразмерная основа кроме нуля делает ВСЁ
                            // объявление невалидным (`flex: 0 0 4` не
                            // применяется вовсе, flexbox_flex-*-unitless-basis).
                            if crate::style::values::value::number(c).is_some_and(|n| n != 0.0) {
                                return;
                            }
                            self.flex_grow = number(a);
                            self.flex_shrink = number(b);
                            // `content` — ключевое слово основы (css-flexbox-1 §7.2),
                            // а не длина: `Len::parse` его не знает, и `flex: 0 0
                            // content` падал в `auto` с заданной шириной
                            // (`flexbox-flex-basis-content-001b/002b`,
                            // `percentage-heights-016`). Смысл тот же, что у длинной
                            // формы `flex-basis: content` ниже.
                            if c.eq_ignore_ascii_case("content") {
                                self.flex_basis = Some(Len::Auto);
                                self.basis_content = Some(true);
                            } else {
                                self.flex_basis = Len::parse(c);
                            }
                        }
                        _ => {}
                    }
                }
            },
            "flex-basis" if v == "content" => {
                self.flex_basis = Some(Len::Auto);
                self.basis_content = Some(true);
            }
            "flex-basis" => {
                if let Some(l) = Len::parse(v)
                    && !matches!(l, Len::Px(x) | Len::Pct(x) if x < 0.0)
                {
                    self.flex_basis = Some(l);
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
                    self.align_self = a;
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
                self.align_inherit &= !ainh::ALIGN_ITEMS;
                if let Ok(a) = align_keyword(v) {
                    self.align_items = a;
                    self.align_items_safe = is_safe(v);
                    self.align_items_last = v.split_whitespace().any(|w| w == "last");
                }
            }
            // `space-evenly` и `space-around` различаются шириной крайних
            // промежутков — сведение их в одно значение расходилось с
            // браузером на 27 точек (поймано сравнением).
            "justify-content" => {
                self.align_inherit &= !ainh::JUSTIFY_CONTENT;
                self.justify_content = parse_justify(v);
                self.justify_content_safe = is_safe(v);
            }
            _ => self.apply_gap_place(key, val, v, hit),
        }
    }
}
