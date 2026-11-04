//! Единственный мост типизированных hooks в жизненный цикл GPUI.

use super::*;

impl<T: Hooks> Element for Hooked<T> {
    fn paint_boundary(&self) -> bool {
        self.0.paint_boundary()
    }
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        self.0.paint_metadata()
    }

    fn paint_async<'a>(
        &'a mut self,
        _: Option<&'a GlobalElementId>,
        _: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &'a mut T::Layout,
        prepaint: &'a mut T::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Hooks::paint_async(&mut self.0, bounds, layout, prepaint, window, cx, stack)
    }

    type RequestLayoutState = T::Layout;
    type PrepaintState = T::Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, T::Layout) {
        Hooks::request_layout(&mut self.0, window, cx)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut T::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> T::Prepaint {
        Hooks::prepaint(&mut self.0, bounds, layout, window, cx)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut T::Layout,
        prepaint: &mut T::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        Hooks::paint(&mut self.0, bounds, layout, prepaint, window, cx);
    }
}
