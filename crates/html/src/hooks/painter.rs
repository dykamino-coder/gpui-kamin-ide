//! Адаптер слоёв без детей к единому жизненному циклу.

use super::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};

/// Слой без детей: геометрия оверлея и работа с кадром, без участия в потоке.
/// Стиль задаётся явно: растянутый inset и размер 100% различаются в GPUI.
pub(crate) trait Painter: 'static {
    type Prepaint: Default + 'static;

    fn style(&self) -> gpui::Style;

    fn prepaint(
        &mut self,
        _layout: LayoutId,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::Prepaint {
        Self::Prepaint::default()
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _layout: &mut LayoutId,
        _prepaint: &mut Self::Prepaint,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
    fn paint_async<'a>(
        &'a mut self,
        bounds: Bounds<Pixels>,
        layout: &'a mut LayoutId,
        prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        _stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            self.paint(bounds, layout, prepaint, window, cx);
        }
    }
}

/// Адаптер paint-only слоя: один layout-id, без детей и состояния самого GPUI.
pub(crate) struct PaintOnly<T>(pub(crate) T);

impl<T: Painter> Hooks for PaintOnly<T> {
    type Layout = LayoutId;
    type Prepaint = T::Prepaint;

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, LayoutId) {
        let id = window.request_layout(self.0.style(), [], cx);
        (id, id)
    }

    async fn prepaint_async(
        &mut self,
        _bounds: Bounds<Pixels>,
        layout: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> T::Prepaint {
        self.0.prepaint(*layout, window, cx)
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut LayoutId,
        prepaint: &mut T::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        super::paint_on_stack(self, bounds, layout, prepaint, window, cx);
    }
    fn paint_async<'a>(
        &'a mut self,
        bounds: Bounds<Pixels>,
        layout: &'a mut LayoutId,
        prepaint: &'a mut T::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        self.0
            .paint_async(bounds, layout, prepaint, window, cx, stack)
    }
}
