//! Разбор списков счётчиков до изменения вычисленного стиля.

use super::*;

/// Проверка всех имён и числовых операндов списка счётчиков.
pub(super) fn counter_value(v: &str, reversed: bool) -> bool {
    if matches!(v, "none" | "inherit") {
        return true;
    }
    let words = split_outside_parens(v);
    let mut words = words.iter().peekable();
    let mut any = false;
    while let Some(word) = words.next() {
        let name = if reversed {
            word.strip_prefix("reversed(")
                .and_then(|w| w.strip_suffix(')'))
                .unwrap_or(word)
        } else {
            word
        };
        if !crate::style::css::selector_tokens::ident(name)
            || matches!(
                name,
                "none" | "default" | "initial" | "inherit" | "unset" | "revert" | "revert-layer"
            )
        {
            return false;
        }
        any = true;
        if let Some(next) = words.peek() {
            let digits = next.strip_prefix(['+', '-']).unwrap_or(next);
            if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit())
                || ["calc(", "min(", "max(", "clamp("]
                    .iter()
                    .any(|p| next.starts_with(p))
            {
                words.next();
            }
        }
    }
    any
}
