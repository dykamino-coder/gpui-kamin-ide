//! Computed::apply_display_flex, хвост цепочки: gap, row-gap/column-gap, order, flex-flow,
//! align-content, justify-items/self, place-*. Ветви в исходном порядке после apply_flex_items.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_gap_place(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        match key {
            "gap" => {
                // Куски — по пробелам ВНЕ скобок: `calc(15% + 7px) calc(10px +
                // 5%)` рвался на шесть кусков, и объявление молча отбрасывалось
                // (`grid-gutters-011/012`). Смесь долей с точками доживает
                // индексом (`parse_mixed`): зазор разрешает её от своей стороны
                // контент-бокса в `paint/apply/` (css-gaps-1 §gap-percent).
                let tokens = split_outside_parens(v);
                if !(1..=2).contains(&tokens.len()) {
                    return;
                }
                let (row, col) = parsed_value!(
                    self,
                    gap_value(&tokens[0], vcx).zip(gap_value(&tokens[tokens.len() - 1], vcx))
                );
                self.gap = Some((row, col));
                self.column_gap = if tokens[tokens.len() - 1] == "normal" {
                    None
                } else {
                    Len::parse(&tokens[tokens.len() - 1], vcx)
                };
            }
            "row-gap" => {
                let parsed = parsed_value!(self, gap_value(v, vcx));
                self.gap = Some((parsed, self.gap.and_then(|g| g.1)));
            }
            "column-gap" => {
                let parsed = parsed_value!(self, gap_value(v, vcx));
                self.gap = Some((self.gap.and_then(|g| g.0), parsed));
                self.column_gap = parsed;
            }
            "order" => self.order = apply_parsed!(self, v.parse().ok()),
            "flex-flow" => {
                let (mut direction, mut wrap) = (None, None);
                for token in v.split_whitespace() {
                    if matches!(token, "row" | "row-reverse" | "column" | "column-reverse") {
                        if direction.replace(token).is_some() {
                            return;
                        }
                    } else if matches!(token, "wrap" | "wrap-reverse" | "nowrap" | "balance") {
                        if wrap.replace(token).is_some() {
                            return;
                        }
                    } else {
                        return;
                    }
                }
                if direction.is_none() && wrap.is_none() {
                    return;
                }
                self.apply_one("flex-direction", direction.unwrap_or("row"), vcx);
                self.apply_one("flex-wrap", wrap.unwrap_or("nowrap"), vcx);
            }
            "align-content" => {
                let parsed = parsed_value!(
                    self,
                    parse_justify(v).map(Some).or_else(|| (v == "normal"
                        || parse_align(v) == Some(Align::Baseline))
                    .then_some(None))
                );
                self.align_inherit &= !ainh::ALIGN_CONTENT;
                self.align_content = parsed;
                self.align_content_baseline = (parse_align(v) == Some(Align::Baseline))
                    .then(|| v.split_whitespace().any(|word| word == "last"));
                self.align_content_safe = is_safe(v);
                self.align_content_unsafe = v.split_whitespace().any(|word| word == "unsafe");
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
                if self.align_content_block || v == "normal" {
                    self.accept_declaration();
                }
            }
            "justify-items" => {
                let parsed = parsed_value!(
                    self,
                    parse_align(v)
                        .map(Some)
                        .or_else(|| (matches!(v, "normal" | "legacy")).then_some(None))
                );
                self.align_inherit &= !ainh::JUSTIFY_ITEMS;
                self.justify_items = parsed;
                if matches!(v, "normal" | "legacy") {
                    self.accept_declaration();
                }
                self.justify_items_safe = is_safe(v);
                self.justify_items_last = v.split_whitespace().any(|w| w == "last");
            }
            "justify-self" => {
                let parsed = parsed_value!(
                    self,
                    parse_align(v)
                        .map(Some)
                        .or_else(|| (matches!(v, "auto" | "normal")).then_some(None))
                );
                self.align_inherit &= !ainh::JUSTIFY_SELF;
                self.justify_self = parsed;
                if matches!(v, "auto" | "normal") {
                    self.accept_declaration();
                }
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
                let (cross, main) = match key {
                    "place-items" => ("align-items", "justify-items"),
                    "place-content" => ("align-content", "justify-content"),
                    _ => ("align-self", "justify-self"),
                };
                // CSS Cascade 5 §7.3: общее слово сокращения переходит в каждый лонгхенд.
                let wide = ["inherit", "initial", "unset", "revert", "revert-layer"]
                    .iter()
                    .any(|k| v.eq_ignore_ascii_case(k));
                let pair = if wide {
                    Some((v, v))
                } else if key == "place-content" && parse_align(v) == Some(Align::Baseline) {
                    Some((v, "start"))
                } else if alignment_value(cross, v) && alignment_value(main, v) {
                    Some((v, v))
                } else {
                    v.char_indices()
                        .filter(|(_, c)| c.is_whitespace())
                        .find_map(|(i, _)| {
                            let (a, b) = (v[..i].trim(), v[i..].trim());
                            (alignment_value(cross, a) && alignment_value(main, b))
                                .then_some((a, b))
                        })
                };
                let Some((a, b)) = pair else {
                    return;
                };
                self.apply_one(cross, a, vcx);
                self.apply_one(main, b, vcx);
            }
            _ => *hit = false,
        }
    }
}

fn gap_value(v: &str, vcx: ValueCx<'_>) -> Option<Option<Len>> {
    if v == "normal" {
        return Some(None);
    }
    size_range::parse_nonnegative_size(v, vcx)
        .filter(|l| {
            !matches!(
                l,
                Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
            )
        })
        .map(Some)
}

fn alignment_value(key: &str, v: &str) -> bool {
    if key.ends_with("content") {
        parse_justify(v).is_some()
            || v == "normal"
            || (key == "align-content" && parse_align(v) == Some(Align::Baseline))
    } else {
        align_keyword(v).is_ok() || (key == "justify-items" && v == "legacy")
    }
}
