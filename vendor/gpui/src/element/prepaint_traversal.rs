//! Продолжение prepaint удерживает состояние Drawable до завершения потомков.

use super::*;

impl<E: Element> Drawable<E> {
    pub(crate) async fn prepaint_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) {
        // KaminIDE patch (#76): self-time профиль prepaint по типам элементов.
        let _prof = cx
            .instrumentation()
            .prepaint
            .enter(std::any::type_name::<E>());
        match &mut self.phase {
            ElementDrawPhase::RequestLayout {
                layout_id,
                global_id,
                inspector_id,
                request_layout,
            }
            | ElementDrawPhase::LayoutComputed {
                layout_id,
                global_id,
                inspector_id,
                request_layout,
                ..
            } => {
                Self::push_element_id(&self.element, global_id.as_ref(), window);

                let bounds = window.layout_bounds(*layout_id);
                let pushed_a11y_node = Self::begin_a11y_prepaint(
                    &mut self.element,
                    global_id.as_ref(),
                    bounds,
                    window,
                );

                let node_id = window.next_frame.dispatch_tree.push_node();
                let mut prepaint = self
                    .element
                    .prepaint_async(
                        global_id.as_ref(),
                        inspector_id.as_ref(),
                        bounds,
                        request_layout,
                        window,
                        cx,
                        stack,
                    )
                    .await;
                window.next_frame.dispatch_tree.pop_node();

                if pushed_a11y_node {
                    Self::end_a11y_prepaint(
                        &mut self.element,
                        global_id.as_ref(),
                        &mut prepaint,
                        window,
                    );
                }

                if global_id.is_some() {
                    window.element_id_stack.pop();
                }

                self.finish_prepaint(node_id, bounds, prepaint);
            }
            _ => panic!("must call request_layout before prepaint"),
        }
    }
}
