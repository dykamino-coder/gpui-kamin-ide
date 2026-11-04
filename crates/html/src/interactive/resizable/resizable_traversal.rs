//! Продолжения запроса и prepaint удерживают состояние до завершения потомка.

use super::*;

impl Resizable {
    pub(super) async fn request_layout_on_stack(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, AnyElement) {
        let build = self.build.clone();
        let Some(global_id) = id else {
            let mut el = build(None, None);
            let layout_id = el.request_layout_async(window, cx, stack).await;
            return (layout_id, el);
        };
        window
            .with_element_state_async::<State, _>(global_id, async |state, window| {
                let st = state.unwrap_or_default();
                let mut el = build(st.width, st.height);
                let layout_id = el.request_layout_async(window, cx, stack).await;
                ((layout_id, el), st)
            })
            .await
    }
    pub(super) async fn prepaint_on_stack(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Hitbox {
        // Область попадания — только уголок: остальная площадь элемента
        // обязана оставаться кликабельной как обычно.
        let grip = Bounds {
            origin: gpui::point(
                bounds.origin.x + bounds.size.width - px(GRIP),
                bounds.origin.y + bounds.size.height - px(GRIP),
            ),
            size: gpui::size(px(GRIP), px(GRIP)),
        };
        crate::interactive::hitbox::prepaint_hitbox_async(child, grip, window, cx, stack).await
    }
}
