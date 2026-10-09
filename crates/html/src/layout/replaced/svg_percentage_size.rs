//! Resolve outer SVG percentage geometry before its viewport is rasterized.

use crate::style::computed::{Computed, Display, Position};
use crate::dom::Element;
use crate::style::values::value::Len;

/// SVG width/height presentation attributes participate in CSS sizing, rather
/// than supplying a natural pixel size (SVG 2 §8.12; CSS 2 §10.5). Blink's
/// SVGSVGElement::CollectStyleForPresentationAttribute likewise collects them
/// only for the outermost SVG. An authored CSS dimension takes precedence.
pub(crate) fn resolve(element: &Element, parent: &Computed) -> Option<Element> {
    if element.tag != "svg"
        || element.style.position == Some(Position::Fixed)
        || parent.inline_display == Some(true)
        || !matches!(
            parent.display,
            None | Some(Display::Block | Display::InlineBlock)
        )
    {
        return None;
    }
    // Fixed positioning needs the viewport or a transformed containing block,
    // neither of which can be inferred from the immediate parent's dimensions.
    let positioned = element.style.position == Some(Position::Absolute);
    if positioned && matches!(parent.position, None | Some(Position::Static)) && !parent.root_box {
        // The DOM parent is not necessarily the positioned containing block.
        return None;
    }
    let percentages = [
        ("width", element.style.width),
        ("height", element.style.height),
    ]
    .map(|(name, dimension)| {
        if dimension.is_some() {
            return None;
        }
        element
            .attr(name)
            .and_then(|value| value.trim().strip_suffix('%'))
            .and_then(|value| value.trim().parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
    });
    if percentages == [None, None] {
        return None;
    }
    let borders = parent.borders();
    let pixels = |length| match length {
        Some(Len::Px(value)) => value,
        _ => 0.0,
    };
    let padding = [
        pixels(parent.padding.left) + pixels(parent.padding.right),
        pixels(parent.padding.top) + pixels(parent.padding.bottom),
    ];
    let border = [
        pixels(borders.left) + pixels(borders.right),
        pixels(borders.top) + pixels(borders.bottom),
    ];
    let padding_is_definite = [
        [parent.padding.left, parent.padding.right],
        [parent.padding.top, parent.padding.bottom],
    ]
    .map(|sides| {
        sides
            .iter()
            .all(|side| matches!(side, None | Some(Len::Px(_))))
    });
    let mut copy = element.clone();
    let mut changed = false;
    for (axis, (base, dimension)) in [
        (parent.width, &mut copy.style.width),
        (parent.height, &mut copy.style.height),
    ]
    .into_iter()
    .enumerate()
    {
        let Some(Len::Px(base)) = base else { continue };
        let Some(fraction) = percentages[axis] else {
            continue;
        };
        if (positioned || parent.border_box == Some(true)) && !padding_is_definite[axis] {
            continue;
        }
        // CSS 2 §10.1: positioned descendants use the padding box;
        // ordinary in-flow descendants use the content box.
        let content = if parent.border_box == Some(true) {
            (base - padding[axis] - border[axis]).max(0.0)
        } else {
            base.max(0.0)
        };
        let basis = content + if positioned { padding[axis] } else { 0.0 };
        *dimension = Some(Len::Px(basis * fraction / 100.0));
        changed = true;
    }
    changed.then_some(copy)
}
