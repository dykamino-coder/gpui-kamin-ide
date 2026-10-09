//! CSS Writing Modes 4 §2.2 maps inline bidi values to Unicode control sequences.
//! Isolated overrides need both nesting levels; plaintext chooses its own first strong direction.
use crate::computed::Computed;

pub fn bidi_marks(
    own: &Computed,
    merged: &Computed,
) -> (Option<&'static str>, Option<&'static str>) {
    let own = if own.bidi_inherit { merged } else { own };
    let rtl = merged.rtl == Some(true);
    if own.bidi_plaintext == Some(true) {
        return (Some("\u{2068}"), Some("\u{2069}"));
    }
    if own.bidi_override == Some(true) {
        if own.bidi_isolate == Some(true) {
            let open = if rtl {
                "\u{2068}\u{202e}"
            } else {
                "\u{2068}\u{202d}"
            };
            return (Some(open), Some("\u{202c}\u{2069}"));
        }
        let open = if rtl { "\u{202e}" } else { "\u{202d}" };
        return (Some(open), Some("\u{202c}"));
    }
    if own.bidi_isolate == Some(true) {
        let open = if rtl { "\u{2067}" } else { "\u{2066}" };
        return (Some(open), Some("\u{2069}"));
    }
    // The embedding uses the computed direction, including an inherited value.
    if own.bidi_embed == Some(true) {
        let open = if rtl { "\u{202b}" } else { "\u{202a}" };
        return (Some(open), Some("\u{202c}"));
    }
    (None, None)
}

/// CSS Writing Modes 4 section 2.2: only explicit inheritance copies the mode.
pub(crate) fn resolve(parent: &Computed, child: &mut Computed) {
    if child.bidi_inherit {
        child.bidi_override = parent.bidi_override;
        child.bidi_isolate = parent.bidi_isolate;
        child.bidi_embed = parent.bidi_embed;
        child.bidi_plaintext = parent.bidi_plaintext;
    }
}
