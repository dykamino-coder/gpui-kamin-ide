//! Parse direction independently of the non-inherited Unicode bidi modes.

use super::Computed;

pub(super) fn apply(style: &mut Computed, key: &str, value: &str) {
    if key == "direction" {
        // CSS Cascade 4 section 7.3.2: inherit replaces the earlier value.
        // The parent direction is resolved by inline::inherit from None.
        style.rtl = match value.to_ascii_lowercase().as_str() {
            "inherit" => None,
            "rtl" => Some(true),
            "ltr" => Some(false),
            _ => style.rtl,
        };
        return;
    }
    style.bidi_override = Some(matches!(value, "bidi-override" | "isolate-override"));
    // Plaintext selects each paragraph's first strong direction; it does
    // not wrap the text in an ordinary directional isolate.
    style.bidi_isolate = Some(matches!(value, "isolate" | "isolate-override"));
    style.bidi_plaintext = Some(value == "plaintext");
    style.bidi_embed = Some(value == "embed");
}
