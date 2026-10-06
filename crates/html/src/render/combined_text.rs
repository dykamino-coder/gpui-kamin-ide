//! Horizontal compositions apply only in vertical writing modes, with no letter spacing.
//! CSS Writing Modes 4 §9.1 and §9.1.2 preserve font settings inside the composition.
use super::{Element, RenderOpts, gather_text, inline, paragraph};
use crate::{
    computed::{Computed, TextTransform},
    dom::Node,
    value::Len,
};
use gpui::IntoElement;
use unicode_segmentation::UnicodeSegmentation;

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
            && !(text.chars().all(|c| c.is_ascii_digit())
                && text.graphemes(true).count() <= count as usize)
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
    merged.line_height = Some(Len::Px(em));
    let feature = match text.graphemes(true).count() {
        2 => Some("hwid"),
        3 => Some("twid"),
        4 => Some("qwid"),
        _ => None,
    };
    if let Some(feature) = feature {
        merged.font_features.push((feature.to_string(), 1));
    }
    let composed;
    let nodes = if element
        .children
        .iter()
        .all(|node| matches!(node, Node::Text(_)))
    {
        let transformed = inline::transform_case(&plain, &merged);
        let multiple = transformed.trim().graphemes(true).count() > 1;
        let text = transformed
            .chars()
            .map(|ch| if multiple { narrow(ch) } else { ch })
            .collect();
        // Width normalization follows text-transform; do not reapply full-width during shaping.
        merged.text_transform = Some(TextTransform::None);
        merged.text_transform_flags = 0;
        composed = vec![Node::Text(text)];
        composed.as_slice()
    } else {
        element.children.as_slice()
    };
    let inner = paragraph(nodes, &merged, opts);
    Some(inline::Piece::Atom(
        crate::interact::CombinedUpright::new(inner, em).into_any_element(),
    ))
}

/// Reverse Unicode full-width compatibility forms before compression (Writing Modes 4 §9.1.3.1).
fn narrow(ch: char) -> char {
    let value = match ch as u32 {
        0x3000 => 0x20,
        0xFF01..=0xFF5E => ch as u32 - 0xFEE0,
        0xFF5F => 0x2985,
        0xFF60 => 0x2986,
        0xFFE0 => 0xA2,
        0xFFE1 => 0xA3,
        0xFFE2 => 0xAC,
        0xFFE3 => 0xAF,
        0xFFE4 => 0xA6,
        0xFFE5 => 0xA5,
        0xFFE6 => 0x20A9,
        _ => return ch,
    };
    char::from_u32(value).unwrap_or(ch)
}
