//! KaminIDE patch: завершение входов стиля из layout до фиксации кадра.
use super::*;

pub(super) struct PrepaintRoots {
    pub(super) root_element: AnyElement,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(super) inspector_element: Option<AnyElement>,
    pub(super) prompt_element: Option<AnyElement>,
    pub(super) active_drag_element: Option<AnyElement>,
    pub(super) tooltip_element: Option<AnyElement>,
}

impl Window {
    /// Повторить стиль/layout до paint этого кадра. Вызывающий публикует
    /// изменённый вход в prepaint и обеспечивает продвижение: например,
    /// size-contained предок стабилизируется раньше своих потомков.
    pub fn request_layout_retry(&mut self) {
        self.invalidator.debug_assert_prepaint();
        self.layout_retry = true;
    }

    pub(super) fn prepaint_roots_with_layout_inputs(
        &mut self,
        root_size: Size<Pixels>,
        _inspector_width: Pixels,
        cx: &mut App,
    ) -> PrepaintRoots {
        let text_index = self.text_system.layout_index();
        loop {
            self.layout_retry = false;
            if self.a11y.is_active() {
                self.a11y.begin_frame();
            }
            let mut root = self.root.as_ref().unwrap().clone().into_any_element();
            let id = root.request_layout(self, cx);
            let scale_factor = self.scale_factor();
            self.layout_engine
                .as_mut()
                .unwrap()
                .stretch_auto_size_to_fill(id, root_size, scale_factor);
            root.prepaint_as_root(Point::default(), root_size.into(), self, cx);
            #[cfg(any(feature = "inspector", debug_assertions))]
            let inspector_element = self.prepaint_inspector(_inspector_width, cx);

            self.prepaint_deferred_draws(cx);

            let mut prompt_element = None;
            let mut active_drag_element = None;
            let mut tooltip_element = None;
            if let Some(prompt) = self.prompt.take() {
                let mut element = prompt.view.any_view().into_any_element();
                let prompt_layout_id = element.request_layout(self, cx);
                self.layout_engine
                    .as_mut()
                    .unwrap()
                    .stretch_auto_size_to_fill(prompt_layout_id, root_size, scale_factor);
                element.prepaint_as_root(Point::default(), root_size.into(), self, cx);
                prompt_element = Some(element);
                self.prompt = Some(prompt);
            } else if let Some(active_drag) = cx.active_drag.take() {
                let mut element = active_drag.view.clone().into_any_element();
                let offset = self.mouse_position() - active_drag.cursor_offset;
                element.prepaint_as_root(offset, AvailableSpace::min_size(), self, cx);
                active_drag_element = Some(element);
                cx.active_drag = Some(active_drag);
            } else {
                tooltip_element = self.prepaint_tooltip(cx);
            }
            if !self.layout_retry {
                return PrepaintRoots {
                    root_element: root,
                    #[cfg(any(feature = "inspector", debug_assertions))]
                    inspector_element,
                    prompt_element,
                    active_drag_element,
                    tooltip_element,
                };
            }

            // Регистрации отброшенной попытки не попадают в кадр; состояние
            // элементов (scroll/focus/view caches) сохраняется. Refresh не
            // позволяет повторно использовать диапазоны отброшенного prepaint.
            let states = mem::take(&mut self.next_frame.element_states);
            self.next_frame.clear();
            self.next_frame.element_states = states;
            self.text_system.truncate_layouts(text_index.clone());
            self.requested_autoscroll = None;
            self.tooltip_bounds = None;
            self.layout_engine.as_mut().unwrap().clear();
            self.refreshing = true;
        }
    }
}
