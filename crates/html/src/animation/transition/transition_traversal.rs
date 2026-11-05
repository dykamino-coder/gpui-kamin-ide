//! Продолжения запроса и prepaint удерживают состояние до завершения потомка.

use super::*;

impl Transition {
    pub(super) async fn request_layout_on_stack(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, AnyElement) {
        let Some(global_id) = id else {
            // Без идентификатора памяти нет — рисуем обычный стиль.
            let mut el = (self.build)(0.0);
            let layout_id = el.request_layout_async(window, cx, stack).await;
            return (layout_id, el);
        };
        let seconds = self.seconds;
        let build = self.build.clone();
        window
            .with_element_state_async::<State, _>(global_id, async |state, window| {
                let mut st = state.unwrap_or_default();
                let now = Instant::now();
                let dt = st
                    .last
                    .map(|t| now.duration_since(t).as_secs_f32())
                    .unwrap_or(0.0)
                    // Потолок шага: после сворачивания окна между кадрами проходят
                    // минуты, и переход обязан доиграть, а не прыгнуть.
                    .min(0.25);
                st.last = Some(now);

                let target = if st.hovered { 1.0 } else { 0.0 };
                let step = dt / seconds;
                if (target - st.progress).abs() <= step {
                    st.progress = target;
                } else {
                    st.progress += step * (target - st.progress).signum();
                }
                if st.progress != target {
                    self.doc.values.defer_frame();
                    window.request_animation_frame();
                }

                let mut el = build(st.progress);
                let layout_id = el.request_layout_async(window, cx, stack).await;
                ((layout_id, el), st)
            })
            .await
    }
    pub(super) async fn prepaint_on_stack(
        &mut self,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Hitbox {
        crate::interactive::hitbox::prepaint_hitbox_async(child, bounds, window, cx, stack).await
    }
}
