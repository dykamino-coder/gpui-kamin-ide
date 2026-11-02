//! Процентные `width`/`height` внешнего `<svg>` сводятся к точкам от родителя до
//! растеризации его viewport: презентационные атрибуты участвуют в CSS-размере (SVG 2
//! §8.12, CSS 2 §10.5), база — content box родителя, у абсолюта — padding box (CSS 2 §10.1);
//! авторская CSS-длина сильнее атрибута.

use crate::dom::Element;
use crate::style::computed::{Computed, Display, Position};
use crate::style::values::value::Len;

/// SVG width/height presentation attributes participate in CSS sizing, rather
/// than supplying a natural pixel size (SVG 2 §8.12; CSS 2 §10.5). Blink's
/// SVGSVGElement::CollectStyleForPresentationAttribute likewise collects them
/// only for the outermost SVG. An authored CSS dimension takes precedence.
pub(crate) fn resolve(element: &Element, parent: &Computed) -> Option<Element> {
    if element.tag != "svg"
        || element.style.used().reset_positioned.position == Some(Position::Fixed)
        || parent.reset_context.inline_display == Some(true)
        || !matches!(
            parent.reset_box_layout.display,
            None | Some(Display::Block | Display::InlineBlock)
        )
    {
        return None;
    }
    // Fixed positioning needs the viewport or a transformed containing block,
    // neither of which can be inferred from the immediate parent's dimensions.
    let positioned = element.style.used().reset_positioned.position == Some(Position::Absolute);
    if positioned
        && matches!(
            parent.reset_positioned.position,
            None | Some(Position::Static)
        )
        && !parent.reset_context.root_box
    {
        // The DOM parent is not necessarily the positioned containing block.
        return None;
    }
    let percentages = [
        ("width", element.style.used().reset_box_layout.width),
        ("height", element.style.used().reset_box_layout.height),
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
        pixels(parent.reset_box_layout.padding.left)
            + pixels(parent.reset_box_layout.padding.right),
        pixels(parent.reset_box_layout.padding.top)
            + pixels(parent.reset_box_layout.padding.bottom),
    ];
    let border = [
        pixels(borders.left) + pixels(borders.right),
        pixels(borders.top) + pixels(borders.bottom),
    ];
    let padding_is_definite = [
        [
            parent.reset_box_layout.padding.left,
            parent.reset_box_layout.padding.right,
        ],
        [
            parent.reset_box_layout.padding.top,
            parent.reset_box_layout.padding.bottom,
        ],
    ]
    .map(|sides| {
        sides
            .iter()
            .all(|side| matches!(side, None | Some(Len::Px(_))))
    });
    let mut copy = element.clone();
    let mut changed = false;
    let style = &mut *copy.style.used_mut();
    for axis in 0..2 {
        let (base, dimension) = match axis {
            0 => (
                parent.reset_box_layout.width,
                &mut style.reset_box_layout.width,
            ),
            _ => (
                parent.reset_box_layout.height,
                &mut style.reset_box_layout.height,
            ),
        };
        let Some(Len::Px(base)) = base else { continue };
        let Some(fraction) = percentages[axis] else {
            continue;
        };
        if (positioned || parent.reset_border.border_box == Some(true))
            && !padding_is_definite[axis]
        {
            continue;
        }
        // CSS 2 §10.1: positioned descendants use the padding box;
        // ordinary in-flow descendants use the content box.
        let content = if parent.reset_border.border_box == Some(true) {
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
