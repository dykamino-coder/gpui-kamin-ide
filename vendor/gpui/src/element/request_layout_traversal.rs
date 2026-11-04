//! Продолжение request_layout удерживает состояние Drawable до завершения потомков.

use super::*;

impl<E: Element> Drawable<E> {
    pub(super) async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> LayoutId {
        match mem::take(&mut self.phase) {
            ElementDrawPhase::Start => {
                let global_id = self
                    .element
                    .id()
                    .map(|element_id| prepare_element_id(element_id, window));

                let inspector_id;
                #[cfg(any(feature = "inspector", debug_assertions))]
                {
                    inspector_id = if window.inspector_enabled() {
                        self.element
                            .source_location()
                            .map(|source| prepare_inspector_id(source, window))
                    } else {
                        None
                    };
                }
                #[cfg(not(any(feature = "inspector", debug_assertions)))]
                {
                    inspector_id = None;
                }

                let (layout_id, request_layout) = self
                    .element
                    .request_layout_async(
                        global_id.as_ref(),
                        inspector_id.as_ref(),
                        window,
                        cx,
                        stack,
                    )
                    .await;

                if global_id.is_some() {
                    window.element_id_stack.pop();
                }

                self.phase = ElementDrawPhase::RequestLayout {
                    layout_id,
                    global_id,
                    inspector_id,
                    request_layout,
                };
                layout_id
            }
            _ => panic!("must call request_layout only once"),
        }
    }
}
