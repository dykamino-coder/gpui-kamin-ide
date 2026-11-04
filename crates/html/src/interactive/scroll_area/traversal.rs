//! Продолжения запроса и prepaint удерживают состояние до завершения потомка.

use super::*;

impl ScrollArea {
    pub(super) async fn request_layout_on_stack(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, <Self as Element>::RequestLayoutState) {
        let build = self.build.clone();
        let (h, v) = (self.horizontal, self.vertical);
        let layout = async |state: Persistent,
                            window: &mut Window,
                            cx: &mut App,
                            stack: &mut gpui::PaintStack| {
            state.extent.set_handle(state.handle.clone());
            let (layout_id, el) = {
                let frame = state.extent.begin(self.tree.as_deref());
                let mut el = build(&state.handle, h, v, frame);
                let layout_id = el.request_layout_async(window, cx, stack).await;
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
            Some(global_id) => {
                window
                    .with_element_state_async::<Persistent, _>(global_id, async |state, window| {
                        layout(state.unwrap_or_default(), window, cx, stack).await
                    })
                    .await
            }
            None => layout(Persistent::default(), window, cx, stack).await.0,
        }
    }
    pub(super) async fn prepaint_on_stack(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut <Self as Element>::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> <Self as Element>::PrepaintState {
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
        child.2.set_decoration_clip(window.content_mask());
        child.2.begin(self.tree.as_deref());
        {
            // CSSOM View §6: retain the fractional scroll translation until
            // HTML paint snaps final edges, rather than snapping the offset
            // separately from each descendant's layout box.
            window
                .with_exact_element_offset_async(
                    gpui::point(gpui::px(0.0), gpui::px(0.0)),
                    async |window| child.0.prepaint_async(window, cx, stack).await,
                )
                .await;
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
        let hitbars = crate::interactive::scrollbars::interaction::prepaint(
            self.freeze_scrollbars,
            &self.style,
            &child.1,
            child.2.bars(),
            child.2.range(),
            child.2.insets(),
            paint_bounds,
            window,
        );
        (paint_bounds, hitbars)
    }
}
