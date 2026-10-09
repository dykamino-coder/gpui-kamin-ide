//! Resolve replaced content's sizing behavior before constructing its inner image.
//! CSS Sizing 3's “behaves as auto” also applies to intrinsic ratio transfer.

use crate::style::computed::Position;
use crate::dom::Element;
use crate::style::values::value::Len;

/// CSS Sizing 4 #aspect-ratio: authored ratios transfer the box-sizing box;
/// natural ratios, including auto <ratio>, always transfer the content box.
pub(crate) fn transfer(
    style: &crate::style::computed::Computed,
    size: f32,
    ratio: f32,
    from_width: bool,
    offsets: [f32; 2],
) -> f32 {
    let [x, y] = if style.aspect_ratio.is_some() && style.border_box == Some(true) {
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
    let indefinite_height = matches!(element.style.height, Some(Len::Pct(_)))
        && !element.style.cb_height_def
        && !element.style.root_box
        && !matches!(
            element.style.position,
            Some(Position::Absolute | Position::Fixed)
        );
    let percentage_limit = containing_width.is_some()
        && matches!(element.style.min_width, Some(Len::Pct(_)))
        || containing_width.is_some() && matches!(element.style.max_width, Some(Len::Pct(_)));
    if !keyword(element.style.width)
        && !keyword(element.style.height)
        && !indefinite_height
        && !percentage_limit
    {
        return None;
    }
    let mut normalized = element.clone();
    if keyword(normalized.style.width) {
        normalized.style.width = None;
    }
    if keyword(normalized.style.height) || indefinite_height {
        normalized.style.height = None;
    }
    // The outer box and its content must constrain the same intrinsic ratio.
    // Leave unresolved bases to layout, rather than substituting an ancestor size.
    if let Some(width) = containing_width {
        for limit in [
            &mut normalized.style.min_width,
            &mut normalized.style.max_width,
        ] {
            if let Some(Len::Pct(fraction)) = *limit {
                *limit = Some(Len::Px(width * fraction));
            }
        }
    }
    Some(normalized)
}

/// CSS 2 sections 10.2 and 10.3.2 resolve an inline replaced width against
/// its containing block, before the synthetic line flex row is constructed.
pub(crate) fn inline_percentage_width(style: &mut crate::style::computed::Computed, basis: Option<f32>) {
    let Some(Len::Pct(fraction)) = style.width else {
        return;
    };
    let Some(basis) = basis.filter(|value| value.is_finite() && *value >= 0.0) else {
        return;
    };
    if matches!(style.position, Some(Position::Absolute | Position::Fixed))
        || style.float.is_some_and(|value| value != 0)
    {
        return;
    }
    style.width = Some(Len::Px(basis * fraction));
    // CSS 2 section 10.3.2: auto horizontal margins on inline replaced
    // boxes have a used value of zero, rather than centering a flex item.
    for margin in [&mut style.margin.left, &mut style.margin.right] {
        if *margin == Some(Len::Auto) {
            *margin = Some(Len::Px(0.0));
        }
    }
    // Margin and padding percentages retain the same CSS containing block
    // after the percentage width becomes a definite size (CSS 2 sections 8.3-8.4).
    for side in [
        &mut style.padding.top,
        &mut style.padding.right,
        &mut style.padding.bottom,
        &mut style.padding.left,
        &mut style.margin.top,
        &mut style.margin.right,
        &mut style.margin.bottom,
        &mut style.margin.left,
    ] {
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
            && crate::render::inline_level(element)
        {
            inline_percentage_width(&mut element.style, basis);
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
        image.style.height = Some(Len::Pct(1.0));
        image.style.max_width = Some(Len::Pct(1.0));
        let normalized = normalize(&image, Some(100.0)).unwrap();
        assert_eq!(normalized.style.height, None);
        assert_eq!(normalized.style.max_width, Some(Len::Px(100.0)));
        assert_eq!(
            crate::layout::replaced::limits::css2_replaced_limits(200.0, 200.0, None, Some(100.0), None, None,),
            (100.0, 100.0)
        );
    }

    #[test]
    fn definite_and_positioned_percentage_heights_keep_their_basis() {
        let mut image = crate::layout::table::anon::anon_element("img", vec![]);
        image.style.height = Some(Len::Pct(0.5));
        image.style.cb_height_def = true;
        assert!(normalize(&image, None).is_none());
        image.style.cb_height_def = false;
        for position in [Position::Absolute, Position::Fixed] {
            image.style.position = Some(position);
            assert!(normalize(&image, None).is_none());
        }
    }

    #[test]
    fn unresolved_width_constraints_keep_percentage_identity() {
        let mut image = crate::layout::table::anon::anon_element("img", vec![]);
        image.style.max_width = Some(Len::Pct(0.5));
        assert!(normalize(&image, None).is_none());
    }
}
