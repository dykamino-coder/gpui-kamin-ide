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
}

/// Адаптер paint-only слоя: один layout-id, без детей и состояния самого GPUI.
pub(crate) struct PaintOnly<T>(pub(crate) T);

impl<T: Painter> Hooks for PaintOnly<T> {
    type Layout = LayoutId;
    type Prepaint = T::Prepaint;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        let id = window.request_layout(self.0.style(), [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        layout: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
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
        self.0.paint(bounds, layout, prepaint, window, cx);
    }
}
