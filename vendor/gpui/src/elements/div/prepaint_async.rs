//! Interactivity удерживает стиль и persistent state в продолжении prepaint.

use super::*;

impl Interactivity {
    /// Подготавливает интерактивность, сохраняя области при приостановке потомков.
    pub async fn prepaint_async<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        content_size: Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        f: impl AsyncFnOnce(
            &Style,
            Point<Pixels>,
            Option<Hitbox>,
            &mut Window,
            &mut App,
            &mut crate::PaintStack,
        ) -> R,
    ) -> R {
        self.content_size = content_size;

        #[cfg(any(feature = "inspector", debug_assertions))]
        window.with_inspector_state(
            _inspector_id,
            cx,
            |inspector_state: &mut Option<DivInspectorState>, _window| {
                if let Some(inspector_state) = inspector_state {
                    inspector_state.bounds = bounds;
                    inspector_state.content_size = content_size;
                }
            },
        );

        if let Some(focus_handle) = self.tracked_focus_handle.as_ref() {
            window.set_focus_handle(focus_handle, cx);

            if window.a11y.is_active() {
                if let Some(global_id) = global_id {
                    let node_id = global_id.accesskit_node_id();
                    window.a11y.set_focusable(node_id, focus_handle.id);
                    if focus_handle.is_focused(window) {
                        window.a11y.set_focus(node_id);
                    }
                } else if focus_handle.is_focused(window) {
                    // Focusable, but with no element id it can't have an
                    // accessibility node, so screen readers fall back to the
                    // whole window.
                    window
                        .a11y
                        .note_focus_without_node(focus_handle.id, "it has no element id");
                }
            }
        }

        if self.report_active_descendant_focus && window.a11y.is_active() {
            if let Some(global_id) = global_id {
                window
                    .a11y
                    .set_active_descendant(global_id.accesskit_node_id());
            }
        }
        if let Some(global_id) = global_id {
            stack
                .run(|stack| self.prepaint_stateful_async(global_id, bounds, window, cx, stack, f))
                .await
        } else {
            self.prepaint_with_state_async(None, bounds, window, cx, stack, f)
                .await
        }
    }

    // KaminIDE patch: anonymous divs must not retain the owned-state branch.
    #[inline(never)]
    async fn prepaint_stateful_async<R>(
        &mut self,
        global_id: &GlobalElementId,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        f: impl AsyncFnOnce(
            &Style,
            Point<Pixels>,
            Option<Hitbox>,
            &mut Window,
            &mut App,
            &mut crate::PaintStack,
        ) -> R,
    ) -> R {
        window
            .with_element_state_async::<InteractiveElementState, _>(
                global_id,
                async |state, window| {
                    let mut state = state.unwrap_or_default();
                    let result = self
                        .prepaint_with_state_async(Some(&mut state), bounds, window, cx, stack, f)
                        .await;
                    (result, state)
                },
            )
            .await
    }

    async fn prepaint_with_state_async<R>(
        &mut self,
        mut element_state: Option<&mut InteractiveElementState>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        f: impl AsyncFnOnce(
            &Style,
            Point<Pixels>,
            Option<Hitbox>,
            &mut Window,
            &mut App,
            &mut crate::PaintStack,
        ) -> R,
    ) -> R {
        let style = self.compute_style_boxed(None, element_state.as_deref_mut(), window, cx);

        if let Some(element_state) = element_state {
            if let Some(clicked_state) = element_state.clicked_state.as_ref() {
                let clicked_state = clicked_state.borrow();
                self.active = Some(clicked_state.element);
            }
            if self
                .rare()
                .is_some_and(|rare| rare.hover_style.is_some() || rare.group_hover_style.is_some())
            {
                element_state
                    .hover_state
                    .get_or_insert_with(Default::default);
            }
            if let Some(active_tooltip) = element_state.active_tooltip.as_ref() {
                if self
                    .rare()
                    .is_some_and(|rare| rare.tooltip_builder.is_some())
                {
                    self.tooltip_id = set_tooltip_on_window(active_tooltip, window);
                } else {
                    // If there is no longer a tooltip builder, remove the active tooltip.
                    if let Some(long_press_tooltip_active) =
                        element_state.long_press_tooltip_active.as_ref()
                    {
                        long_press_tooltip_active.set(false);
                    }
                    element_state.active_tooltip.take();
                }
            }
        }

        window
            .with_child_style_async(&style, bounds, async |window| {
                let hitbox = if self.should_insert_hitbox(&style, window, cx) {
                    Some(window.insert_hitbox(bounds, self.hitbox_behavior))
                } else {
                    None
                };
                let scroll_offset = self.clamp_scroll_position(bounds, &style, window, cx);
                f(&style, scroll_offset, hitbox, window, cx, stack).await
            })
            .await
    }
}
