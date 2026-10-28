//! Фиксация строчной оси измеряемой вертикальной копии.

use gpui::{IntoElement, ParentElement, Styled};

pub(super) fn wrap(
    element: gpui::AnyElement,
    vertical: bool,
    reverse: bool,
    inline: Option<f32>,
) -> gpui::AnyElement {
    // Строчную ось фиксирует колонка у каждой копии вертикальной стопки: мера
    // ребёнка — раскладка его копии, независимо от того, что пишет рекордер.
    let Some(inline) = inline.filter(|_| vertical) else {
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
