//! Явный запрос выбирает отдельную мономорфизацию prepaint, без флага в кадре.

use super::PageStack;
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};

pub(crate) struct GeometryPages(pub(crate) PageStack);

impl Hooks for GeometryPages {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        self.0.request_layout(window, cx)
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        window.with_render_context(
            Some(std::rc::Rc::new(
                crate::layout::geometry::fragments::BlockAxis(self.0.axis.is_vertical()),
            )),
            |window| self.0.prepaint_for::<true>(bounds, window, cx),
        );
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut (),
        prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(bounds, layout, prepaint, window, cx);
    }
}
