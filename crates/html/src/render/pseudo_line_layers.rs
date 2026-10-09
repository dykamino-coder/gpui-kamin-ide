//! Install a block container's own first-line and first-letter layers.

use super::{Computed, Element, Len};

/// CSS 2 sections 5.12.1-5.12.2 apply to block containers, including
/// inline-blocks and table cells. These specialized render paths must seed
/// their contents with the same layers as the ordinary block path.
pub(crate) fn install(element: &Element, merged: &mut Computed) {
    install_first_letter(element, merged);
    merged.first_line = element
        .first_line
        .as_ref()
        .map(|l| resolve(l, merged.font_size));
}

pub(crate) fn install_first_letter(element: &Element, merged: &mut Computed) {
    merged.first_letter = element
        .first_letter
        .as_ref()
        .map(|l| resolve(l, merged.font_size));
}

fn resolve(layer: &Computed, base: Option<Len>) -> Box<Computed> {
    let mut layer = layer.clone();
    if let (Some(Len::Px(base)), Some(Len::Pct(factor))) = (base, layer.font_size) {
        layer.font_size = Some(Len::Px(base * factor));
    }
    // Leave em values for inline::max_font_size, which resolves them
    // against the paragraph's parent rather than scaling them twice.
    Box::new(layer)
}
