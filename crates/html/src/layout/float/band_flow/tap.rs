//! Tap: обёртка ребёнка, сообщающая его итоговые границы раскладке полос.

use crate::hooks::Hooks;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Прозрачная обёртка, запоминающая `LayoutId` ребёнка: после пробной
/// раскладки каркаса по нему читается размер самого ребёнка, а не каркаса.
pub(super) struct Tap {
    pub(super) inner: AnyElement,
    pub(super) id: Rc<Cell<Option<LayoutId>>>,
}

impl Hooks for Tap {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let id = self.inner.request_layout(window, cx);
        self.id.set(Some(id));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        // A probe wrapper is not a stacking context: the child's inline
        // foreground stays in the enclosing paint phases (CSS 2.1 Appendix
        // E.2 steps 4, 5, 7).
        gpui::paint_reopen(|| self.inner.paint(window, cx));
    }
}
