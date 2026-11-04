//! Продолжения paint хранят стиль, обработчики и области предка вне native стека.

use super::*;

impl Interactivity {
    pub(super) async fn paint_async(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        hitbox: Option<&Hitbox>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        f: impl AsyncFnOnce(&Style, &mut Window, &mut App, &mut crate::PaintStack),
    ) {
        self.hovered = hitbox.map(|hitbox| hitbox.is_hovered(window));
        if let Some(global_id) = global_id {
            self.paint_stateful_async(
                global_id,
                _inspector_id,
                bounds,
                hitbox,
                window,
                cx,
                stack,
                f,
            )
            .await;
        } else {
            self.paint_with_state_async(
                None,
                None,
                _inspector_id,
                bounds,
                hitbox,
                window,
                cx,
                stack,
                f,
            )
            .await;
        }
    }

    // Keep owned state out of anonymous divs' recursive paint frames.
    #[inline(never)]
    async fn paint_stateful_async(
        &mut self,
        global_id: &GlobalElementId,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        hitbox: Option<&Hitbox>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        f: impl AsyncFnOnce(&Style, &mut Window, &mut App, &mut crate::PaintStack),
    ) {
        window
            .with_element_state_async::<InteractiveElementState, _>(
                global_id,
                async |state, window| {
                    let mut state = state.unwrap_or_default();
                    self.paint_with_state_async(
                        Some(&mut state),
                        Some(global_id),
                        inspector_id,
                        bounds,
                        hitbox,
                        window,
                        cx,
                        stack,
                        f,
                    )
                    .await;
                    ((), state)
                },
            )
            .await;
    }

    async fn paint_with_state_async(
        &mut self,
        mut element_state: Option<&mut InteractiveElementState>,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        hitbox: Option<&Hitbox>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        f: impl AsyncFnOnce(&Style, &mut Window, &mut App, &mut crate::PaintStack),
    ) {
        let style = self.compute_style_boxed(hitbox, element_state.as_deref_mut(), window, cx);

        #[cfg(any(test, all(debug_assertions, feature = "test-support")))]
        if let Some(debug_selector) = &self.debug_selector {
            window
                .next_frame
                .record_debug_bounds(debug_selector.clone(), bounds);
        }

        self.paint_hover_group_handler(window, cx);

        if style.visibility == Visibility::Hidden {
            return;
        }

        let mut tab_group = None;
        if self.tab_group {
            tab_group = self.tab_index;
        }

        window
            .with_element_opacity_async(style.opacity, async |window| {
                style.paint_box_with(bounds, self.box_painter.as_ref(), window, cx);
                style
                    .paint_after_box_async(window, cx, async |window: &mut Window, cx: &mut App| {
                        window
                            .with_child_style_async(&style, bounds, async |window| {
                                window
                                    .with_tab_group_async(tab_group, async |window| {
                                        // Register the container's own focus handle *inside* its
                                        // tab group, so that focusing the container and then
                                        // calling `focus_next` descends into this group's first
                                        // item. Inserting it before `with_tab_group` would give the
                                        // container a shallower tab path than its children; with
                                        // sibling groups every container would then sort ahead of
                                        // every item, and `focus_next` from a container would jump
                                        // to the first item in the whole window instead of its own.
                                        if let Some(focus_handle) = &self.tracked_focus_handle {
                                            window.next_frame.tab_stops.insert(focus_handle);
                                        }
                                        if let Some(hitbox) = hitbox {
                                            #[cfg(debug_assertions)]
                                            self.paint_debug_info(
                                                global_id, hitbox, &style, window, cx,
                                            );

                                            if let Some(drag) = cx.active_drag.as_ref() {
                                                if let Some(mouse_cursor) = drag.cursor_style {
                                                    window.set_window_cursor_style(mouse_cursor);
                                                }
                                            } else {
                                                if let Some(mouse_cursor) = style.mouse_cursor {
                                                    window.set_cursor_style(mouse_cursor, hitbox);
                                                }
                                            }

                                            if let Some(group) = self.group.clone() {
                                                GroupHitboxes::push(group, hitbox.id, cx);
                                            }

                                            if let Some(area) =
                                                self.rare().and_then(|rare| rare.window_control)
                                            {
                                                window.insert_window_control_hitbox(
                                                    area,
                                                    hitbox.clone(),
                                                );
                                            }

                                            self.paint_mouse_listeners(
                                                hitbox,
                                                element_state.as_deref_mut(),
                                                window,
                                                cx,
                                            );
                                            self.paint_scroll_listener(hitbox, &style, window, cx);
                                        }

                                        self.paint_keyboard_listeners(window, cx);

                                        if window.a11y.is_active() {
                                            if let Some(global_id) = global_id {
                                                let a11y_action_listeners =
                                                    self.take_listeners(|l| {
                                                        &mut l.a11y_action_listeners
                                                    });
                                                if !a11y_action_listeners.is_empty() {
                                                    let node_id = global_id.accesskit_node_id();
                                                    for (action, listener) in a11y_action_listeners
                                                    {
                                                        window.on_a11y_action(
                                                            node_id, action, listener,
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        f(&style, window, cx, stack).await;

                                        if let Some(_hitbox) = hitbox {
                                            #[cfg(any(feature = "inspector", debug_assertions))]
                                            window.insert_inspector_hitbox(
                                                _hitbox.id,
                                                _inspector_id,
                                                cx,
                                            );

                                            if let Some(group) = self.group.as_ref() {
                                                GroupHitboxes::pop(group, cx);
                                            }
                                        }
                                    })
                                    .await
                            })
                            .await;
                    })
                    .await;
            })
            .await;
    }
}
