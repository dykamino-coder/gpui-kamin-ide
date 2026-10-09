//! Parse mask dimensions without duplicating a single value into both axes.
use super::Computed;
use crate::value::Len;

pub(super) fn apply(style: &mut Computed, value: &str) {
    let (size, fit) = match value.trim() {
        "contain" => (None, 1),
        "cover" => (None, 2),
        "auto" | "auto auto" => (None, 0),
        value => {
            let words = crate::background::split_top(value);
            if !(1..=2).contains(&words.len()) {
                return;
            }
            let Some(width) = Len::parse(words[0]) else {
                return;
            };
            let height = if let Some(word) = words.get(1) {
                let Some(height) = Len::parse(word) else {
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
    style.mask_size = size;
    style.mask_fit = Some(fit);
}
