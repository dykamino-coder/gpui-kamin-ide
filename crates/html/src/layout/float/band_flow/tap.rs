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

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout) {
        let id = self.inner.request_layout_async(window, cx, stack).await;
        self.id.set(Some(id));
        (id, ())
    }

    async fn prepaint_async(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        self.inner.prepaint_async(window, cx, stack).await;
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, _bounds, _state, _prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _bounds: Bounds<Pixels>,
        _state: &'a mut Self::Layout,
        _prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(async move {
            // A probe wrapper is not a stacking context: the child's inline
            // foreground stays in the enclosing paint phases (CSS 2.1 Appendix
            // E.2 steps 4, 5, 7).
            crate::hooks::paint_reopen_async(window, async |window| {
                self.inner.paint_async(window, cx, stack).await
            })
            .await;
        })
    }
}
