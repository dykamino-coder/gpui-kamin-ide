//! Продолжение Div сохраняет порядок и области дочерней фазы prepaint.

use super::*;

impl Div {
    pub(super) async fn prepaint_on_stack(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut DivFrameState,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) -> Option<Hitbox> {
        // KaminIDE patch: stale descendants cannot publish inputs in this pass.
        if let Some(observer) = &mut self.layout_observer
            && !observer.prepaint(window)
        {
            return None;
        }
        let image_cache = self
            .image_cache
            .as_mut()
            .map(|provider| provider.provide(window, cx));

        let has_prepaint_listener = self.prepaint_listener.is_some();
        let mut children_bounds = Vec::with_capacity(if has_prepaint_listener {
            request_layout.child_layout_ids.len()
        } else {
            0
        });

        let mut child_min = point(Pixels::MAX, Pixels::MAX);
        let mut child_max = Point::default();
        if let Some(handle) = self.interactivity.scroll_anchor.as_ref() {
            *handle.last_origin.borrow_mut() = bounds.origin - window.element_offset();
        }
        let content_size = if request_layout.child_layout_ids.is_empty() {
            bounds.size
        } else if let Some(scroll_handle) = self.interactivity.tracked_scroll_handle.as_ref() {
            let mut state = scroll_handle.0.borrow_mut();
            state.child_bounds = Vec::with_capacity(request_layout.child_layout_ids.len());
            for child_layout_id in &request_layout.child_layout_ids {
                let child_bounds = window.layout_bounds(*child_layout_id);
                child_min = child_min.min(&child_bounds.origin);
                child_max = child_max.max(&child_bounds.bottom_right());
                state.child_bounds.push(child_bounds);
            }
            (child_max - child_min).into()
        } else {
            for child_layout_id in &request_layout.child_layout_ids {
                let child_bounds = window.layout_bounds(*child_layout_id);
                child_min = child_min.min(&child_bounds.origin);
                child_max = child_max.max(&child_bounds.bottom_right());

                if has_prepaint_listener {
                    children_bounds.push(child_bounds);
                }
            }
            (child_max - child_min).into()
        };

        if let Some(scroll_handle) = self.interactivity.tracked_scroll_handle.as_ref() {
            scroll_handle.scroll_to_active_item();
        }

        // KaminIDE patch: unrounded bounds of this node, only when they lie
        // within half a device pixel of the snapped ones (a node moved by
        // `prepaint_at` has no meaningful layout position).
        request_layout.exact_bounds = request_layout.layout_id.and_then(|id| {
            let origin = window.layout_origin_unrounded(id);
            let size = window.layout_size_unrounded(id);
            let exact = Bounds { origin, size };
            let half = px(0.5 / window.scale_factor().max(0.01) + 1e-4);
            let near = |a: Pixels, b: Pixels| (a - b).abs() <= half;
            (near(exact.left(), bounds.left())
                && near(exact.top(), bounds.top())
                && near(exact.right(), bounds.right())
                && near(exact.bottom(), bounds.bottom()))
            .then_some(exact)
        });
        let outer_exact = std::mem::replace(
            &mut window.raster_frame.exact_bounds,
            request_layout.exact_bounds.map(|exact| (bounds, exact)),
        );

        let hitbox = self
            .interactivity
            .prepaint_async(
                global_id,
                inspector_id,
                bounds,
                content_size,
                window,
                cx,
                stack,
                async |style, scroll_offset, hitbox, window, cx, stack| {
                    // Children bring their own exact bounds.
                    window.raster_frame.exact_bounds = None;
                    // skip children
                    if style.display == Display::None {
                        return hitbox;
                    }

                    window
                        .with_image_cache_async(image_cache, async |window| {
                            window
                                .with_element_offset_async(scroll_offset, async |window| {
                                    if let Some(traversal) =
                                        window.render_context::<crate::PrepaintTraversal>()
                                    {
                                        (traversal.enter_children)(window);
                                    }
                                    if let Some(order_fn) = &self.prepaint_order_fn {
                                        let order = order_fn(window, cx);
                                        for idx in order {
                                            if let Some(child) = self.children.get_mut(idx) {
                                                child.prepaint_async(window, cx, stack).await;
                                            }
                                        }
                                    } else {
                                        for child in &mut self.children {
                                            child.prepaint_async(window, cx, stack).await;
                                        }
                                    }
                                })
                                .await;

                            if let Some(listener) = self.prepaint_listener.as_ref() {
                                listener(children_bounds, window, cx);
                            }
                        })
                        .await;

                    hitbox
                },
            )
            .await;
        window.raster_frame.exact_bounds = outer_exact;
        hitbox
    }
}
