//! Вычисленный стиль: разбор `direction` и `unicode-bidi` в поля `Computed`.
//! `direction` наследуется (`inherit` сбрасывает в `None`, родителя подставит
//! наследование), режимы `unicode-bidi` — нет; `inherit` заменяет каждую часть
//! значения (CSS Cascade 4 §7.3.2), поэтому оба свойства разбираются независимо.

use super::Computed;

pub(super) fn apply(style: &mut Computed, key: &str, value: &str) {
    if key == "direction" {
        if matches!(value.to_ascii_lowercase().as_str(), "ltr" | "rtl") {
            style.accept_declaration();
        }
        // CSS Cascade 4 section 7.3.2: inherit replaces the earlier value.
        // The parent direction is resolved by style::cascade::inherit from None.
        style.rtl = match value.to_ascii_lowercase().as_str() {
            "inherit" => None,
            "rtl" => Some(true),
            "ltr" => Some(false),
            _ => style.rtl,
        };
        return;
    }
    let lower = value.to_ascii_lowercase();
    let value = lower.as_str();
    if !matches!(
        value,
        "normal"
            | "embed"
            | "isolate"
            | "bidi-override"
            | "isolate-override"
            | "plaintext"
            | "inherit"
            | "initial"
            | "unset"
    ) {
        return;
    }
    // CSS Cascade 4 section 7.3.2: inherit replaces every part of the value.
    style.accept_declaration();
    style.bidi_inherit = value == "inherit";
    style.bidi_override = Some(matches!(value, "bidi-override" | "isolate-override"));
    style.bidi_isolate = Some(matches!(value, "isolate" | "isolate-override"));
    style.bidi_plaintext = Some(value == "plaintext");
    style.bidi_embed = Some(value == "embed");
}
