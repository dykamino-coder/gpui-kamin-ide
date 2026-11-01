//! Используемый стиль замещаемого до сборки его внутренней картинки: ключевые слова
//! содержимого и неопределённые доли высоты ведут себя как `auto` (css-sizing-3), и это
//! распространяется на перенос природной пропорции (css-sizing-4 #aspect-ratio: авторская
//! пропорция переносит коробку `box-sizing`, природная — всегда content box); процентная
//! ширина строчной замещаемой — от содержащего блока до построения ряда строки (CSS 2 §10.3.2).

use crate::dom::Element;
use crate::style::computed::Position;
use crate::style::values::value::Len;

/// CSS Sizing 4 #aspect-ratio: authored ratios transfer the box-sizing box;
/// natural ratios, including auto <ratio>, always transfer the content box.
pub(super) fn transfer(
    style: &crate::style::computed::Computed,
    size: f32,
    ratio: f32,
    from_width: bool,
    offsets: [f32; 2],
) -> f32 {
    let [x, y] = if style.reset_shapes.aspect_ratio.is_some()
        && style.reset_border.border_box == Some(true)
    {
        offsets
    } else {
        [0.0, 0.0]
    };
    if from_width {
        ((size + x) / ratio - y).max(0.0)
    } else {
        ((size + y) * ratio - x).max(0.0)
    }
}

pub(crate) fn normalize(element: &Element, containing_width: Option<f32>) -> Option<Element> {
    let keyword = |length| {
        matches!(
            length,
            Some(Len::MinContent | Len::MaxContent | Len::FitContent)
        )
    };
    let indefinite_height = matches!(
        element.style.used().reset_box_layout.height,
        Some(Len::Pct(_))
    ) && !element.style.used().reset_context.cb_height_def_hint
        && !element.style.used().reset_context.root_box
        && !matches!(
            element.style.used().reset_positioned.position,
            Some(Position::Absolute | Position::Fixed)
        );
    let percentage_limit = containing_width.is_some()
        && matches!(
            element.style.used().reset_box_layout.min_width,
            Some(Len::Pct(_))
        )
        || containing_width.is_some()
            && matches!(
                element.style.used().reset_box_layout.max_width,
                Some(Len::Pct(_))
            );
    if !keyword(element.style.used().reset_box_layout.width)
        && !keyword(element.style.used().reset_box_layout.height)
        && !indefinite_height
        && !percentage_limit
    {
        return None;
    }
    let mut normalized = element.clone();
    if keyword(normalized.style.used().reset_box_layout.width) {
        normalized.style.used_mut().reset_box_layout.width = None;
    }
    if keyword(normalized.style.used().reset_box_layout.height) || indefinite_height {
        normalized.style.used_mut().reset_box_layout.height = None;
    }
    // The outer box and its content must constrain the same intrinsic ratio.
    // Leave unresolved bases to layout, rather than substituting an ancestor size.
    if let Some(width) = containing_width {
        let style = &mut *normalized.style.used_mut();
        for style_field_index in 0..2 {
            let limit = match style_field_index {
                0 => &mut style.reset_box_layout.min_width,
                _ => &mut style.reset_box_layout.max_width,
            };
            if let Some(Len::Pct(fraction)) = *limit {
                *limit = Some(Len::Px(width * fraction));
            }
        }
    }
    Some(normalized)
}

/// CSS 2 sections 10.2 and 10.3.2 resolve an inline replaced width against
/// its containing block, before the synthetic line flex row is constructed.
pub(crate) fn inline_percentage_width(
    style: &mut crate::style::computed::Computed,
    basis: Option<f32>,
) {
    let Some(Len::Pct(fraction)) = style.reset_box_layout.width else {
        return;
    };
    let Some(basis) = basis.filter(|value| value.is_finite() && *value >= 0.0) else {
        return;
    };
    if matches!(
        style.reset_positioned.position,
        Some(Position::Absolute | Position::Fixed)
    ) || style.reset_box_layout.float.is_some_and(|value| value != 0)
    {
        return;
    }
    style.reset_box_layout.width = Some(Len::Px(basis * fraction));
    // CSS 2 section 10.3.2: auto horizontal margins on inline replaced
    // boxes have a used value of zero, rather than centering a flex item.
    for style_field_index in 0..2 {
        let margin = match style_field_index {
            0 => &mut style.reset_box_layout.margin.left,
            _ => &mut style.reset_box_layout.margin.right,
        };
        if *margin == Some(Len::Auto) {
            *margin = Some(Len::Px(0.0));
        }
    }
    // Margin and padding percentages retain the same CSS containing block
    // after the percentage width becomes a definite size (CSS 2 sections 8.3-8.4).
    for style_field_index in 0..8 {
        let side = match style_field_index {
            0 => &mut style.reset_box_layout.padding.top,
            1 => &mut style.reset_box_layout.padding.right,
            2 => &mut style.reset_box_layout.padding.bottom,
            3 => &mut style.reset_box_layout.padding.left,
            4 => &mut style.reset_box_layout.margin.top,
            5 => &mut style.reset_box_layout.margin.right,
            6 => &mut style.reset_box_layout.margin.bottom,
            _ => &mut style.reset_box_layout.margin.left,
        };
        if let Some(Len::Pct(fraction)) = *side {
            *side = Some(Len::Px(basis * fraction));
        }
    }
}

/// Freeze the CSS containing block before float adapters create flex wrappers.
/// Their width is the remaining line window, not the percentage sizing basis.
pub(crate) fn inline_nodes(
    mut nodes: Vec<crate::dom::Node>,
    basis: Option<f32>,
) -> Vec<crate::dom::Node> {
    for node in &mut nodes {
        if let crate::dom::Node::Element(element) = node
            && element.tag == "img"
            && crate::layout::box_kind::inline_level(element)
        {
            inline_percentage_width(element.style.used_mut(), basis);
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indefinite_percentage_height_transfers_max_width_through_intrinsic_ratio() {
        let mut image = crate::layout::table::anon::anon_element("img", vec![]);
        image.style.used_mut().reset_box_layout.height = Some(Len::Pct(1.0));
        image.style.used_mut().reset_box_layout.max_width = Some(Len::Pct(1.0));
        let normalized = normalize(&image, Some(100.0)).unwrap();
        assert_eq!(normalized.style.used().reset_box_layout.height, None);
        assert_eq!(
            normalized.style.used().reset_box_layout.max_width,
            Some(Len::Px(100.0))
        );
        assert_eq!(
            crate::layout::replaced::limits::css2_replaced_limits(
                200.0,
                200.0,
                None,
                Some(100.0),
                None,
                None,
            ),
            (100.0, 100.0)
        );
    }

    #[test]
    fn definite_and_positioned_percentage_heights_keep_their_basis() {
        let mut image = crate::layout::table::anon::anon_element("img", vec![]);
        image.style.used_mut().reset_box_layout.height = Some(Len::Pct(0.5));
        image.style.used_mut().reset_context.cb_height_def_hint = true;
        assert!(normalize(&image, None).is_none());
        image.style.used_mut().reset_context.cb_height_def_hint = false;
        for position in [Position::Absolute, Position::Fixed] {
            image.style.used_mut().reset_positioned.position = Some(position);
            assert!(normalize(&image, None).is_none());
        }
    }

    #[test]
    fn unresolved_width_constraints_keep_percentage_identity() {
        let mut image = crate::layout::table::anon::anon_element("img", vec![]);
        image.style.used_mut().reset_box_layout.max_width = Some(Len::Pct(0.5));
        assert!(normalize(&image, None).is_none());
    }
}
