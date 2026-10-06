//! Horizontal compositions apply only in vertical writing modes, with no letter spacing.
//! CSS Writing Modes 4 §9.1 and §9.1.2 preserve font settings inside the composition.
use super::{Element, RenderOpts, gather_text, inline, paragraph};
use crate::{computed::Computed, value::Len};
use gpui::IntoElement;

pub(super) fn piece(
    element: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<inline::Piece> {
    if inherited.rotated_line != Some(true)
        || inherited.sideways == Some(true)
        || element.style.display.is_some()
    {
        return None;
    }
    let mut merged = inline::inherit(inherited, &element.style);
    let count = merged.combine_upright?;
    let mut plain = String::new();
    gather_text(&element.children, &mut plain);
    let text = plain.trim();
    if text.is_empty()
        || count != 0
            && !(text.chars().all(|c| c.is_ascii_digit()) && text.chars().count() <= count as usize)
    {
        return None;
    }
    merged.combine_upright = None;
    // An explicit zero overrides inherited spacing in nested inline descendants.
    merged.letter_spacing = Some(Len::Px(0.0));
    let em = match merged.font_size {
        Some(Len::Px(value)) => value,
        _ => opts.base_size(),
    };
    let feature = match text.chars().count() {
        2 => Some("hwid"),
        3 => Some("twid"),
        4 => Some("qwid"),
        _ => None,
    };
    if let Some(feature) = feature {
        merged.font_features.push((feature.to_string(), 1));
    }
    let inner = paragraph(&element.children, &merged, opts);
    Some(inline::Piece::Atom(
        crate::interact::CombinedUpright::new(inner, em).into_any_element(),
    ))
}
