//! Measure rotated CSS contributions before device-pixel rounding.
//! Rounded min/max contributions must not change intrinsic size or wrapping.
use crate::computed::orthogonal::{InlineConstraint, InlineKeyword};
use gpui::{AnyElement, App, AvailableSpace, Pixels, Size, Window, px, size};

pub(super) fn measure(
    child: &mut AnyElement,
    constraint: InlineConstraint,
    keyword: Option<InlineKeyword>,
    window: &mut Window,
    cx: &mut App,
) -> Size<Pixels> {
    let max = child.layout_as_root_unrounded(
        size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
        window,
        cx,
    );
    let min = child.layout_as_root_unrounded(
        size(AvailableSpace::MinContent, AvailableSpace::MaxContent),
        window,
        cx,
    );
    let used = px(match keyword {
        Some(keyword) => {
            constraint.used_keyword(Some(keyword), f32::from(min.width), f32::from(max.width))
        }
        None => constraint.used(f32::from(min.width), f32::from(max.width)),
    });
    let laid_out = child.layout_as_root_unrounded(
        size(AvailableSpace::Definite(used), AvailableSpace::MaxContent),
        window,
        cx,
    );
    size(used, laid_out.height)
}
