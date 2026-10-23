//! Вычисленный стиль: разбор `unicode-bidi` в поля `Computed` (`direction` —
//! длинное свойство реестра, `props::inherited`). Режимы не наследуются;
//! `inherit` заменяет каждую часть значения (CSS Cascade 4 §7.3.2).

use super::Computed;

pub(super) fn apply(style: &mut Computed, value: &str) {
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
