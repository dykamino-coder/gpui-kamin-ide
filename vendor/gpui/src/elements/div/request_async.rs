//! Состояние interactivity остаётся в продолжении запроса раскладки.

use super::*;

impl Interactivity {
    pub(super) async fn request_layout_boxed_async(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        f: impl AsyncFnOnce(
            ComputedStyle,
            Option<&Rc<StyleRefinement>>,
            &mut Window,
            &mut App,
            &mut crate::PaintStack,
        ) -> LayoutId,
    ) -> LayoutId {
        #[cfg(any(feature = "inspector", debug_assertions))]
        window.with_inspector_state(
            _inspector_id,
            cx,
            |inspector_state: &mut Option<DivInspectorState>, _window| {
                if let Some(inspector_state) = inspector_state {
                    self.base_style = Rc::from(inspector_state.base_style.clone());
                } else {
                    *inspector_state = Some(DivInspectorState {
                        base_style: Box::new((*self.base_style).clone()),
                        bounds: Default::default(),
                        content_size: Default::default(),
                    })
                }
            },
        );

        window
            .with_optional_element_state_async::<InteractiveElementState, _>(
                global_id,
                async |element_state, window| {
                    let mut element_state =
                        element_state.map(|element_state| element_state.unwrap_or_default());

                    if let Some(element_state) = element_state.as_ref()
                        && cx.has_active_drag()
                    {
                        if let Some(pending_mouse_down) = element_state.pending_mouse_down.as_ref()
                        {
                            *pending_mouse_down.borrow_mut() = None;
                        }
                        if let Some(clicked_state) = element_state.clicked_state.as_ref() {
                            *clicked_state.borrow_mut() = ElementClickedState::default();
                        }
                    }

                    // Ensure we store a focus handle in our element state if we're focusable.
                    // If there's an explicit focus handle we're tracking, use that. Otherwise
                    // create a new handle and store it in the element state, which lives for as
                    // as frames contain an element with this id.
                    if self.focusable
                        && self.tracked_focus_handle.is_none()
                        && let Some(element_state) = element_state.as_mut()
                    {
                        let mut handle = element_state
                            .focus_handle
                            .get_or_insert_with(|| cx.focus_handle())
                            .clone()
                            .tab_stop(self.tab_stop);

                        if let Some(index) = self.tab_index {
                            handle = handle.tab_index(index);
                        }

                        self.tracked_focus_handle = Some(handle);
                    }

                    if let Some(scroll_handle) = self.tracked_scroll_handle.as_ref() {
                        let scroll_handle_state = scroll_handle.0.borrow();
                        self.scroll_offset = Some(scroll_handle_state.offset.clone());
                        self.ongoing_scroll = Some(scroll_handle_state.ongoing_scroll.clone());
                    } else if (self.base_style.overflow.x == Some(Overflow::Scroll)
                        || self.base_style.overflow.y == Some(Overflow::Scroll))
                        && let Some(element_state) = element_state.as_mut()
                    {
                        self.scroll_offset = Some(
                            element_state
                                .scroll_offset
                                .get_or_insert_with(Rc::default)
                                .clone(),
                        );
                        self.ongoing_scroll = Some(
                            element_state
                                .ongoing_scroll
                                .get_or_insert_with(|| {
                                    Rc::new(RefCell::new(OngoingScroll::default()))
                                })
                                .clone(),
                        );
                    }

                    let style = self.compute_style_boxed(None, element_state.as_mut(), window, cx);
                    let refinement = self.rare.is_none().then_some(&self.base_style);
                    let layout_id = f(style, refinement, window, cx, stack).await;
                    (layout_id, element_state)
                },
            )
            .await
    }
}
