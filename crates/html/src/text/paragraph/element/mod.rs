//! impl Element/IntoElement for Paragraph: раскладка, prepaint, отрисовка.

mod inline_anchor;
mod layout;
mod painting;
mod prepaint;

use crate::text::paragraph::*;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hitbox, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

impl Element for Paragraph {
    type RequestLayoutState = LayoutId;
    /// Область попадания заводится только у выделяемого абзаца.
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        reblessive::Stack::new()
            .enter(|stack| self.request_layout_impl(_id, _inspector_id, window, cx, stack))
            .finish()
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<Hitbox> {
        reblessive::Stack::new()
            .enter(|stack| self.prepaint_impl(bounds, state, window, _cx, stack))
            .finish()
    }

    fn request_layout_async<'a>(
        &'a mut self,
        id: Option<&'a GlobalElementId>,
        inspector: Option<&'a InspectorElementId>,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a, (LayoutId, LayoutId)> {
        Box::pin(self.request_layout_impl(id, inspector, window, cx, stack))
    }

    fn prepaint_async<'a>(
        &'a mut self,
        _id: Option<&'a GlobalElementId>,
        _inspector: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &'a mut LayoutId,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a, Option<Hitbox>> {
        Box::pin(self.prepaint_impl(bounds, state, window, cx, stack))
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        reblessive::Stack::new()
            .enter(|stack| {
                self.paint_impl(id, _inspector_id, bounds, _state, hitbox, window, cx, stack)
            })
            .finish();
    }
    fn paint_async<'a>(
        &'a mut self,
        id: Option<&'a GlobalElementId>,
        inspector: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &'a mut LayoutId,
        hitbox: &'a mut Option<Hitbox>,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(self.paint_impl(id, inspector, bounds, state, hitbox, window, cx, stack))
    }
}

impl IntoElement for Paragraph {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
