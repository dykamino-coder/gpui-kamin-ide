//! Отдельный query-элемент выбирает точную геометрию, не меняя обычный кадр.

use super::ColumnStack;
use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, Bounds, IntoElement, LayoutId, Pixels, Window};

impl ColumnStack {
    pub(crate) fn into_element_for<const QUERY: bool>(self) -> AnyElement {
        if QUERY {
            GeometryColumns(self).into_hooked().into_any_element()
        } else {
            self.into_hooked().into_any_element()
        }
    }
}

struct GeometryColumns(ColumnStack);
impl Hooks for GeometryColumns {
    type Layout = LayoutId;
    type Prepaint = Bounds<Pixels>;
    fn request_layout(&mut self, w: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        self.0.request_layout(w, cx)
    }
    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut LayoutId,
        w: &mut Window,
        cx: &mut App,
    ) -> Bounds<Pixels> {
        w.with_render_context(
            Some(std::rc::Rc::new(
                crate::layout::geometry::fragments::BlockAxis(self.0.axis.is_vertical()),
            )),
            |w| self.0.prepaint_for::<true>(*id, w, cx),
        )
    }
    fn paint(
        &mut self,
        b: Bounds<Pixels>,
        id: &mut LayoutId,
        p: &mut Bounds<Pixels>,
        w: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(b, id, p, w, cx)
    }
}
