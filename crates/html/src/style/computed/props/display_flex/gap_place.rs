//! Computed::apply_display_flex, хвост цепочки: gap, row-gap/column-gap, order, flex-flow, align-content, justify-items/self, place-*. Ветви в исходном порядке после apply_flex_items.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_gap_place(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "gap" => {
                // Куски — по пробелам ВНЕ скобок: `calc(15% + 7px) calc(10px +
                // 5%)` рвался на шесть кусков, и объявление молча отбрасывалось
                // (`grid-gutters-011/012`). Смесь долей с точками доживает
                // индексом (`parse_mixed`): зазор разрешает её от своей стороны
                // контент-бокса в `apply.rs` (css-gaps-1 §gap-percent).
                let tokens = split_outside_parens(v);
                let parts: Vec<Option<Len>> = tokens.iter().map(|t| Len::parse_mixed(t)).collect();
                self.gap = match parts.len() {
                    1 => Some((parts[0], parts[0])),
                    2 => Some((parts[0], parts[1])),
                    _ => self.gap,
                };
                // Короткая форма задаёт и `column-gap` многоколоночника
                // (css-align-3 §8.3: `gap` = `row-gap` + `column-gap`).
                // Колонки читают только `column_gap` (`render.rs` `used_gap`,
                // `column_flow`), и `gap: 20px 0` прежде оставлял кегль —
                // `column-wrap-no-constraints-001`, красная полоса между
                // колонками.
                // Многоколоночнику — прежний разбор без смеси: его зазор долю
                // с точками не читает.
                if parts.len() == 1 || parts.len() == 2 {
                    self.column_gap = Len::parse(&tokens[tokens.len() - 1]);
                }
            }
            "row-gap" => self.gap = Some((Len::parse(v), self.gap.and_then(|g| g.1))),
            // Одно свойство служит двум раскладкам: в сетке и гибкой строке
            // это зазор между ячейками, в многоколоночном потоке — между
            // колонками. Пишем в оба поля, читает нужное та раскладка, которая
            // включена.
            "column-gap" => {
                self.gap = Some((self.gap.and_then(|g| g.0), Len::parse(v)));
                self.column_gap = Len::parse(v);
            }
            "order" => self.order = v.parse().ok(),
            "flex-flow" => {
                for token in v.split_whitespace() {
                    let prop = if token.starts_with("wrap") || token == "nowrap" {
                        "flex-wrap"
                    } else {
                        "flex-direction"
                    };
                    self.apply_one(prop, token);
                }
            }
            "align-content" => {
                self.align_inherit &= !ainh::ALIGN_CONTENT;
                self.align_content = parse_justify(v);
                self.align_content_safe = is_safe(v);
                // css-align-3 §align-block: ЛЮБОЕ не-`normal` значение делает
                // блочный контейнер корнем блочного контекста форматирования.
                // `parse_justify` этого не покажет: `baseline`/`first`/`last`
                // дают `None` так же, как `normal`. Приставка `safe`/`unsafe`
                // снимается — она про переполнение, а не про значение.
                let word = v
                    .split_whitespace()
                    .find(|w| !matches!(*w, "safe" | "unsafe"))
                    .unwrap_or("");
                self.align_content_block =
                    self.align_content.is_some() || matches!(word, "baseline" | "first" | "last");
            }
            "justify-items" => {
                self.align_inherit &= !ainh::JUSTIFY_ITEMS;
                self.justify_items = parse_align(v);
                self.justify_items_safe = is_safe(v);
                self.justify_items_last = v.split_whitespace().any(|w| w == "last");
            }
            "justify-self" => {
                self.align_inherit &= !ainh::JUSTIFY_SELF;
                self.justify_self = parse_align(v);
                self.justify_self_physical = match v.split_whitespace().last() {
                    Some("left") => Some(false),
                    Some("right") => Some(true),
                    _ => None,
                };
                self.justify_self_normal = v.trim() == "normal";
                self.justify_self_safe = is_safe(v);
                self.justify_self_own_axis = matches!(
                    v.split_whitespace().last(),
                    Some("self-start") | Some("self-end")
                );
                self.justify_self_last = v.split_whitespace().any(|w| w == "last");
            }
            // `place-*` — сокращения «поперёк / вдоль»; одно значение задаёт обе оси.
            "place-items" | "place-content" | "place-self" => {
                let (a, b) = match v.split_once(char::is_whitespace) {
                    Some((a, b)) => (a.trim(), b.trim()),
                    None => (v, v),
                };
                let (cross, main) = match key {
                    "place-items" => ("align-items", "justify-items"),
                    "place-content" => ("align-content", "justify-content"),
                    _ => ("align-self", "justify-self"),
                };
                self.apply_one(cross, a);
                self.apply_one(main, b);
            }
            _ => *hit = false,
        }
    }
}
