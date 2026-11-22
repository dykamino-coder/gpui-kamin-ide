//! Жизненный цикл нативной области прокрутки: владелец extent сохраняется между layout и paint.
use super::*;
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
        gpui::Hitbox,
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
        let build = self.build.clone();
        let (h, v) = (self.horizontal, self.vertical);
        let layout = |mut state: Persistent, window: &mut Window, cx: &mut App| {
            if let Some(root) = &self.root {
                state.handle = root.handle.clone();
                root.axes.set([h, v]);
            }
            state.extent.set_handle(state.handle.clone());
            let (layout_id, el) = {
                let frame = state.extent.begin(self.tree.as_deref());
                let mut el = build(&state.handle, h, v, frame);
                let layout_id = el.request_layout(window, cx);
                (layout_id, el)
            };
            (
                (
                    layout_id,
                    (
                        el,
                        state.handle.clone(),
                        state.extent.clone(),
                        layout_id,
                        state.interaction.clone(),
                    ),
                ),
                state,
            )
        };
        match id {
            Some(global_id) => window
                .with_element_state::<Persistent, _>(global_id, |state, window| {
                    layout(state.unwrap_or_default(), window, cx)
                }),
            None => layout(Persistent::default(), window, cx).0,
        }
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // A frame that replays a retained record runs no layout: it keeps the
        // element states its prepaint range touched (`Window::reuse_prepaint`).
        // Touch the scroller's state here too, or the next build starts it
        // over (a new handle at offset zero) after a replayed wheel step.
        // The root's handle lives with the view (`RootScroll`), not here.
        if let Some(id) = id.filter(|_| self.root.is_none()) {
            window.with_element_state::<Persistent, _>(id, |state, _| {
                ((), state.unwrap_or_default())
            });
        }
        // Match CSS border painting: inset the unrounded box, not a rounded
        // border box. A relocated prepaint_at node must retain its new bounds.
        let exact = Bounds {
            origin: window.layout_origin_unrounded(child.3),
            size: window.layout_size_unrounded(child.3),
        };
        let half = gpui::px(0.5 / window.scale_factor().max(0.01) + 1e-4);
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= half;
        let paint_bounds = if near(exact.left(), bounds.left())
            && near(exact.top(), bounds.top())
            && near(exact.right(), bounds.right())
            && near(exact.bottom(), bounds.bottom())
        {
            exact
        } else {
            bounds
        };
        if let (Some(parent), Some(tree)) = (&self.parent_extent, self.tree.as_deref()) {
            parent.record_border_box(
                tree.node_id,
                &self.style,
                paint_bounds,
                self.sticky.as_ref(),
            );
        }
        let hitbox = window.insert_hitbox(paint_bounds, gpui::HitboxBehavior::Normal);
        child.2.set_decoration_clip(window.content_mask());
        child.2.begin(self.tree.as_deref());
        {
            // CSSOM View §6: retain the fractional scroll translation until
            // HTML paint snaps final edges, rather than snapping the offset
            // separately from each descendant's layout box.
            window.with_exact_element_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)), |window| {
                child.0.prepaint(window, cx)
            });
        }
        if let Some(tree) = self.tree.as_deref()
            && child.2.settle(
                tree,
                &self.style,
                &child.1,
                paint_bounds,
                self.sticky.as_ref(),
                self.values.vcx(),
            )
        {
            // Invalidation during prepaint is ignored by GPUI. Rebuild after
            // this frame, once the native extent proxy has its measured size.
            self.values.vcx().doc.defer_frame();
            let view = window.current_view_opt();
            window.on_next_frame(move |window, cx| {
                if let Some(view) = view {
                    cx.notify(view);
                }
                window.refresh();
            });
        }
        let bars = RootBars {
            style: self.style.clone(),
            freeze: self.freeze_scrollbars,
            handle: child.1.clone(),
            extent: child.2.clone(),
            interaction: child.4.clone(),
            bounds: paint_bounds,
        };
        // The root scrollport's bars follow the offset: the document's frame
        // element draws them after its record each frame (`RootBars`).
        let hitbars = match self.root.as_ref().filter(|root| root.hosted.get()) {
            Some(root) => {
                root.bars.replace(Some(Rc::new(bars)));
                Vec::new()
            }
            None => bars.prepaint(window),
        };
        (paint_bounds, hitbox, hitbars)
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
        child.2.set_decoration_clip(window.content_mask());
        // CSS2 Appendix E.2: scrolling does not establish a stacking context.
        // Preserve its block-background / inline-foreground phases through
        // this native adapter (Blink box_fragment_painter.cc:877-959).
        // Register before descendants: bubble dispatch runs the inner adapter first.
        super::wheel::listen(paint_bounds.1.clone(), child.1.clone(), window);
        crate::hooks::paint_reopen(window, |window| child.0.paint(window, cx));
        if self.root.as_ref().is_some_and(|root| root.hosted.get()) {
            return;
        }
        let alpha = child
            .4
            .paint(self.freeze_scrollbars, &paint_bounds.2, &child.1, window);
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
    }
}

impl IntoElement for ScrollArea {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
