//! Computed::apply_display_flex, хвост цепочки: row-gap/column-gap, order, align-content,
//! justify-items/self. Ветви в исходном порядке после apply_flex_items.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_gap_place(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        match key {
            "row-gap" => {
                let parsed = parsed_value!(self, super::values::gap(v, vcx));
                self.gap = Some((parsed, self.gap.and_then(|g| g.1)));
            }
            "column-gap" => {
                let parsed = parsed_value!(self, super::values::gap(v, vcx));
                self.gap = Some((self.gap.and_then(|g| g.0), parsed));
                self.column_gap = parsed;
            }
            "order" => self.order = apply_parsed!(self, v.parse().ok()),
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
                // `legacy` (с `left`/`right`/`center` или без) своего
                // выравнивания не несёт: поле пусто, решает раскладка.
                let legacy = v.split_whitespace().any(|w| w == "legacy");
                let parsed = parsed_value!(
                    self,
                    parse_align(v)
                        .map(Some)
                        .or_else(|| (v == "normal" || legacy).then_some(None))
                );
                self.align_inherit &= !ainh::JUSTIFY_ITEMS;
                self.justify_items = parsed;
                if v == "normal" || legacy {
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
            _ => *hit = false,
        }
    }
}
