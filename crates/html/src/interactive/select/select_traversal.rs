//! Продолжения запроса и prepaint удерживают состояние до завершения потомка.

use super::*;

impl Selectable {
    pub(super) async fn request_layout_on_stack(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, AnyElement) {
        let selection = match id {
            Some(global_id) => window.with_element_state::<State, _>(global_id, |state, _| {
                let st = state.unwrap_or_default();
                (st.range(), st)
            }),
            None => ((0, 0), State::default()).0,
        };
        let styled = StyledText::new(self.text.clone())
            .with_ligature_breaking(false)
            .with_runs(self.runs_with_selection(selection.0, selection.1));
        // Раскладка нужна на этапе отрисовки, чтобы попасть точкой в символ.
        self.layout = Some(styled.layout().clone());
        let mut el = styled.into_any_element();
        let layout_id = el.request_layout_async(window, cx, stack).await;
        (layout_id, el)
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
