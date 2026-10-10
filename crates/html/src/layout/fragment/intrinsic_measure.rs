//! Intrinsic column contributions use layout sizes before paint rounding.

use gpui::{AnyElement, App, AvailableSpace, Window, size};

pub(crate) fn width(
    element: &mut AnyElement,
    available: AvailableSpace,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    f32::from(
        element
            .layout_as_root_unrounded(size(available, AvailableSpace::MaxContent), window, cx)
            .width,
    )
}
