//! Фаза корня ждёт раскладку на общем явном стеке.
use super::*;
impl<E: Element> Drawable<E> {
    pub(crate) async fn layout_as_root_async(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut PaintStack,
    ) -> Size<Pixels> {
        if matches!(&self.phase, ElementDrawPhase::Start) {
            self.request_layout_async(window, cx, stack).await;
        }

        let layout_id = match mem::take(&mut self.phase) {
            ElementDrawPhase::RequestLayout {
                layout_id,
                global_id,
                inspector_id,
                request_layout,
            } => {
                window
                    .compute_layout_async(layout_id, available_space, cx, stack)
                    .await;
                self.phase = ElementDrawPhase::LayoutComputed {
                    layout_id,
                    global_id,
                    inspector_id,
                    available_space,
                    request_layout,
                };
                layout_id
            }
            ElementDrawPhase::LayoutComputed {
                layout_id,
                global_id,
                inspector_id,
                available_space: prev_available_space,
                request_layout,
            } => {
                if available_space != prev_available_space {
                    window
                        .compute_layout_async(layout_id, available_space, cx, stack)
                        .await;
                }
                self.phase = ElementDrawPhase::LayoutComputed {
                    layout_id,
                    global_id,
                    inspector_id,
                    available_space,
                    request_layout,
                };
                layout_id
            }
            _ => panic!("cannot measure after painting"),
        };

        window.layout_bounds(layout_id).size
    }
}
