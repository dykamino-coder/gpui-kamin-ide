//! Фиксация строчной оси измеряемой вертикальной копии.

use crate::dom::Element;
use gpui::{IntoElement, ParentElement, Styled};

pub(super) fn wrap(
    element: gpui::AnyElement,
    copy: &Element,
    vertical: bool,
    reverse: bool,
    inline: Option<f32>,
) -> gpui::AnyElement {
    let Some(inline) =
        inline.filter(|_| vertical && crate::layout::fragment::flow::vertical_copy(copy, reverse))
    else {
        return element;
    };
    let wrapper = gpui::div().flex().h(gpui::px(inline));
    let wrapper = if reverse {
        wrapper.flex_row_reverse()
    } else {
        wrapper.flex_row()
    };
    wrapper.child(element).into_any_element()
}
