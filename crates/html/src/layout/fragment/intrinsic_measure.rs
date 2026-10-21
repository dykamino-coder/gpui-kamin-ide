//! Ширина элемента для вклада в размер колонки (`multicol::column_measure`): корневая
//! раскладка по max-content без округления к пикселям краски, чтобы вклад совпадал с
//! размером раскладки.

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
