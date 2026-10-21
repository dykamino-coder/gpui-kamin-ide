//! Вычисленный стиль: `mask-size` — `contain`/`cover`/`auto` или одна-две
//! длины; одиночное значение задаёт ширину, а высота остаётся `auto`
//! (CSS Masking §7.8 → CSS Backgrounds §3.9), не дублируясь на обе оси.

use super::Computed;
use crate::document::state::ValueCx;
use crate::style::values::value::Len;

pub(super) fn apply(style: &mut Computed, value: &str, vcx: ValueCx<'_>) {
    let (size, fit) = match value.trim() {
        "contain" => (None, 1),
        "cover" => (None, 2),
        "auto" | "auto auto" => (None, 0),
        value => {
            let words = crate::style::css::value_tokens::split_top(value);
            if !(1..=2).contains(&words.len()) {
                return;
            }
            let Some(width) = Len::parse(words[0], vcx) else {
                return;
            };
            let height = if let Some(word) = words.get(1) {
                let Some(height) = Len::parse(word, vcx) else {
                    return;
                };
                height
            } else {
                // CSS Masking §7.8 -> CSS Backgrounds §3.9: omitted height is auto.
                Len::Auto
            };
            (Some((width, height)), 0)
        }
    };
    style.accept_declaration();
    style.mask_size = size;
    style.mask_fit = Some(fit);
}
