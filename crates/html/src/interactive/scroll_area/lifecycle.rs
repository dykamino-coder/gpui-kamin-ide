//! Жизненный цикл нативной области прокрутки: владелец extent сохраняется между layout и paint.
use super::*;
#[path = "traversal.rs"]
mod traversal;

use gpui::{
    App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels,
    Window,
};
impl Element for ScrollArea {
    type RequestLayoutState = (
        AnyElement,
        gpui::ScrollHandle,
        crate::interactive::scroll_extent::Frame,
        LayoutId,
        crate::interactive::scrollbars::interaction::Interaction,
    );
    type PrepaintState = (
        Bounds<Pixels>,
        Vec<crate::interactive::scrollbars::interaction::HitBar>,
    );

    crate::hooks::keyed_element_identity!();

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        reblessive::Stack::new()
            .enter(|stack| self.request_layout_async(id, _inspector_id, window, cx, stack))
            .finish()
    }
    fn request_layout_async<'a>(
        &'a mut self,
        id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a, (LayoutId, Self::RequestLayoutState)> {
        Box::pin(self.request_layout_on_stack(id, _inspector_id, window, cx, stack))
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        reblessive::Stack::new()
            .enter(|stack| {
                self.prepaint_async(_id, _inspector_id, bounds, child, window, cx, stack)
            })
            .finish()
    }
    fn prepaint_async<'a>(
        &'a mut self,
        _id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &'a mut Self::RequestLayoutState,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a, Self::PrepaintState> {
        Box::pin(self.prepaint_on_stack(bounds, child, window, cx, stack))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        paint_bounds: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        reblessive::Stack::new()
            .enter(|stack| {
                self.paint_async(
                    _id,
                    _inspector_id,
                    _bounds,
                    child,
                    paint_bounds,
                    window,
                    cx,
                    stack,
                )
            })
            .finish();
    }

    fn paint_async<'a>(
        &'a mut self,
        _id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &'a mut Self::RequestLayoutState,
        paint_bounds: &'a mut Self::PrepaintState,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(async move {
            child.2.set_decoration_clip(window.content_mask());
            // CSS2 Appendix E.2: scrolling does not establish a stacking context.
            // Preserve its block-background / inline-foreground phases through
            // this native adapter (Blink box_fragment_painter.cc:877-959).
            crate::hooks::paint_reopen_async(window, async |window| {
                child.0.paint_async(window, cx, stack).await;
            })
            .await;
            let alpha = child
                .4
                .paint(self.freeze_scrollbars, &paint_bounds.1, &child.1, window);
            crate::interactive::scrollbars::paint(
                &self.style,
                &child.1,
                child.2.bars(),
                child.2.range(),
                child.2.insets(),
                paint_bounds.0,
                alpha,
                window,
            );
        })
    }
}

impl IntoElement for ScrollArea {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
