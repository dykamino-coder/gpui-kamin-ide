//! Потомки передают управление явному стеку между уровнями дерева.

use super::*;

impl AnyElement {
    /// Планирует запрос потомка, сохраняя native стек ограниченным.
    pub async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> LayoutId {
        stack
            .run(|stack| self.0.request_layout_async(window, cx, stack))
            .await
    }
    /// Планирует prepaint потомка с сохранением focus-протокола.
    pub async fn prepaint_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Option<FocusHandle> {
        let focus_assigned = window.next_frame.focus.is_some();

        stack
            .run(|stack| self.0.prepaint_async(window, cx, stack))
            .await;

        if !focus_assigned && let Some(focus_id) = window.next_frame.focus {
            return FocusHandle::for_id(focus_id, &cx.focus_handles);
        }

        None
    }
}
