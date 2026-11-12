//! Геометрический запрос вертикального текста не округляет рамку под пиксельную сетку.
use super::VerticalText;
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
struct Query(VerticalText);
impl VerticalText {
    pub(crate) fn into_element_for<const QUERY: bool>(self) -> AnyElement {
        if QUERY {
            Query(self).hooked()
        } else {
            self.hooked()
        }
    }
}
impl Wrap for Query {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.0.child
    }
    fn request_layout(&mut self, w: &mut Window, cx: &mut App) -> LayoutId {
        self.0.request_layout_impl(w, cx)
    }
    fn prepaint(&mut self, b: Bounds<Pixels>, w: &mut Window, cx: &mut App) {
        self.0.prepaint_impl_for::<true>(b, w, cx);
    }
    fn paint(&mut self, b: Bounds<Pixels>, w: &mut Window, cx: &mut App) {
        self.0.paint_impl(b, w, cx);
    }
}
