//! Кусок `text-combine-upright` при сборе абзаца: только в вертикальном письме без
//! `sideways`, при `digits N` — лишь цифры не длиннее N; внутри композиции нулевой
//! `letter-spacing`, высота строки в кегль, возможности `hwid`/`twid`/`qwid` по числу знаков и
//! сужение полноширинных форм (css-writing-modes-4 §9.1, §9.1.2, §9.1.3.1). На выходе атом
//! `CombinedUpright` с внутренним абзацем.

use crate::dom::text_content::gather_text;
use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, TextTransform};
use crate::style::values::value::Len;
use crate::text::inline;
use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn piece(
    element: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<inline::Piece> {
    let vcx = opts.cx.vcx();
    if !inherited.wm().turned() || inherited.wm().sideways() || element.style.display.is_some() {
        return None;
    }
    let mut merged = inherit(inherited, &element.style, vcx);
    let count = merged.combine_upright.flatten()?;
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
    merged.combine_upright = Some(None);
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
    let inner = opts.cx.build.paragraph(nodes, &merged, opts);
    Some(inline::Piece::Atom(
        crate::text::vertical::CombinedUpright::new(inner, em).hooked(),
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
