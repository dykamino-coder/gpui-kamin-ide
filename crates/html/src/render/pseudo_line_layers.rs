//! Install a block container's own first-line and first-letter layers.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// CSS 2 sections 5.12.1-5.12.2 apply to block containers, including
/// inline-blocks and table cells. These specialized render paths must seed
/// their contents with the same layers as the ordinary block path.
pub(crate) fn install(element: &Element, merged: &mut Computed, vcx: ValueCx<'_>) {
    install_first_letter(element, merged);
    merged.first_line = element.first_line.as_ref().map(|layer| {
        // CSS Pseudo 4 §2.1.3: inherit the block's computed values,
        // then cascade the pseudo's own declarations. Its old snapshot
        // still contains the block's specified em font size.
        let own = element.style.first_line_own.as_deref().unwrap_or(layer);
        Box::new(crate::style::cascade::inherit::inherit(merged, own, vcx))
    });
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
