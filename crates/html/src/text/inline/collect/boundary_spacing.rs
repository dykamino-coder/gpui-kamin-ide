//! Boundary spacing for collect; split out to keep the owning module within 250 lines.

use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline::*;

/// Зазор предка за строчной коробкой, ушедшей в раскладку (см. `collect`).
/// Только НЕатомарный строчный элемент без замещения: у атома и картинки
/// межбуквенного интервала по краям Blink не ставит (интервал живёт в наборе
/// текста, `shape_result.cc` `ApplySpacing`).
pub(super) fn boundary_gap_after_box(e: &Element, inherited: &Computed) -> Option<f32> {
    let inline_level = e.style.display.is_none() || e.style.inline_display == Some(true);
    let replaced = matches!(
        e.tag.as_str(),
        "img"
            | "svg"
            | "canvas"
            | "video"
            | "embed"
            | "object"
            | "iframe"
            | "input"
            | "button"
            | "select"
            | "textarea"
    );
    if !inline_level || replaced || e.style.ruby_role.is_some() {
        return None;
    }
    let size = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let gap = crate::text::metrics::spacing_px(
        inherited.letter_spacing,
        &inherited.font_family.clone().unwrap_or_default(),
        size,
    );
    (gap != 0.0).then_some(gap)
}

/// Зазор на границе элементов — на последний ЗНАЧАЩИЙ кусок набора.
///
/// Распорка строчной коробки пропускается: в её трекинге лежит поле коробки, а
/// не межбуквенный интервал, и перебивать его нельзя.
pub(super) fn set_boundary_spacing(pieces: &mut [Piece], spacing: Option<Len>) {
    let last = pieces.iter_mut().rev().find_map(|p| match p {
        Piece::Text { text, style } if !text.is_empty() && text != SPACER => Some(style),
        _ => None,
    });
    if let Some(style) = last {
        style.letter_spacing_after = spacing;
    }
}
