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
        crate::interactive::scroll_animation::Scroll,
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
            state
                .extent
                .set_handle(state.handle.clone(), window.scale_factor());
            let animation = self.animator(&mut state);
            let (layout_id, el) = {
                let frame = state.extent.begin(self.tree.as_deref());
                let mut el = super::timed_build(&build, &state.handle, (h, v), frame);
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
                        animation,
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
        let paint_bounds = self.paint_bounds(bounds, child.3, window);
        child.5.recorded(window.scroll_interest_margin());
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
        // The bars layout decided for this scrollport (laying only it out
        // again when its content asked for others) hold before its content
        // is prepainted: its gutters, insets and painted bars follow them.
        if let Some(bars) = window.layout_scrollbars(child.3) {
            let before = child.2.bars();
            if crate::interactive::scroll_extent::classic::adopt(&child.2, &self.style, bars) {
                // DevTools `why`: a scrollport whose bars layout changed.
                let content = window.layout_content_size_unrounded(child.3);
                let size = window.layout_size_unrounded(child.3);
                crate::why::note("scroll_extent.layout_bars", || {
                    // The descendants carrying the overflow on each axis whose bar appeared.
                    let reach = |axis: usize| {
                        (bars[axis] && !before[axis])
                            .then(|| window.layout_overflow_reach(child.3, axis == 1).join(" > "))
                    };
                    format!(
                        "box {} bars {before:?} -> {bars:?} decided in layout: content {content:?} box {size:?}; reach x [{}] y [{}]; sized by [{}]",
                        self.tree.as_deref().map_or(0, |t| t.node_id),
                        reach(0).unwrap_or_default(),
                        reach(1).unwrap_or_default(),
                        window.layout_ancestry(child.3, 3).join(" < "),
                    )
                });
            }
        }
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
            // Layout decides the bars of most scrollports itself (`adopt`
            // above). Bars it did not decide (`classic::layout_axes` says
            // why, and `settle` notes it) change the content box this layout
            // placed: lay the roots out again in this draw with them. One bar
            // can bring the other, so each layout again publishes the next of
            // the frame's inputs, each once per draw: bars that still change
            // after them wait for the next frame, and so does a frame no state
            // keeps (it would start each layout without its bars).
            if id.is_some()
                && child
                    .2
                    .layout_inputs()
                    .into_iter()
                    .any(|input| window.update_layout_input(input, || true))
            {
                return (paint_bounds, hitbox, Vec::new());
            }
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
        let hitbars = self.prepaint_bars(child, paint_bounds, window);
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
        let hidden = Some(crate::style::computed::Overflow::Hidden);
        let layout = &self.style.reset_box_layout;
        let wheel = [layout.overflow_x != hidden, layout.overflow_y != hidden];
        super::wheel::listen(
            paint_bounds.1.clone(),
            child.1.clone(),
            child.5.clone(),
            wheel,
            window,
        );
        crate::hooks::paint_reopen(window, |window| child.0.paint(window, cx));
        if self.root.as_ref().is_some_and(|root| root.hosted.get()) {
            return;
        }
        let alpha = child.4.paint(
            self.freeze_scrollbars,
            &paint_bounds.2,
            &child.1,
            &child.5,
            window,
        );
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
