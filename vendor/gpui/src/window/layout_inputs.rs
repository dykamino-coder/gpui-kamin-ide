//! KaminIDE patch: завершение входов стиля из layout до фиксации кадра.
use super::*;

/// Идентичность постоянного источника layout-dependent style input.
/// Копии одной коробки используют тот же идентификатор.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LayoutInputId(u64);

impl Default for LayoutInputId {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Retries of one draw's prepaint: an input changes once per draw, so chains
/// of inputs (a container query inside another) end; the limit bounds the
/// draw's cost for a long chain.
const MAX_LAYOUT_RETRIES: usize = 8;

pub(super) struct PrepaintRoots {
    /// `None` when the previous root tree is kept (`kept_root`).
    pub(super) root_element: Option<AnyElement>,
    pub(super) root_prepaint: Range<PrepaintStateIndex>,
    pub(super) kept_root: Option<RootRanges>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(super) inspector_element: Option<AnyElement>,
    pub(super) prompt_element: Option<AnyElement>,
    pub(super) active_drag_element: Option<AnyElement>,
    pub(super) tooltip_element: Option<AnyElement>,
}

impl Window {
    /// Опубликовать вход один раз за кадр, после layout его источника.
    /// Предок обязан отложить prepaint потомков, если вход изменился:
    /// тогда они будут обработаны только с окончательным стилем предка.
    /// Изолированный prepaint вне сборки кадра входы не публикует.
    ///
    /// Every input that changed in one attempt shares one retry of the
    /// attempt. A retry is recorded where it was asked for (`dirty_sources`,
    /// kind `layout-retry`), as a frame request is.
    #[track_caller]
    pub fn update_layout_input(
        &mut self,
        id: LayoutInputId,
        update: impl FnOnce() -> bool,
    ) -> bool {
        self.invalidator.debug_assert_prepaint();
        let Some(inputs) = self.layout_inputs.as_mut() else {
            return false;
        };
        if !inputs.insert(id) {
            return false;
        }
        let changed = update();
        if changed {
            crate::dirty_sources::note("layout-retry", std::panic::Location::caller());
        }
        self.layout_retry |= changed;
        changed
    }

    /// Test support: the last draw asked for a frame after it
    /// (`on_next_frame`), for example to settle what its layout measured.
    #[cfg(any(test, feature = "test-support"))]
    pub fn frame_requested_after_draw(&self) -> bool {
        !self.next_frame_callbacks.borrow().is_empty()
    }

    pub(super) fn prepaint_roots_with_layout_inputs(
        &mut self,
        root_size: Size<Pixels>,
        _inspector_width: Pixels,
        cx: &mut App,
    ) -> PrepaintRoots {
        let text_index = self.text_system.layout_index();
        self.layout_inputs = Some(FxHashSet::default());
        let kept_root = self.kept_root(cx);
        let mut retries = 0;
        loop {
            self.layout_retry = false;
            if self.a11y.is_active() {
                self.a11y.begin_frame();
            }
            let scale_factor = self.scale_factor();
            // KaminIDE patch: when only isolated views changed, the previous
            // root tree is replayed instead of rendered (`window/isolated.rs`).
            let kept_root = kept_root.clone().filter(|_| !self.refreshing);
            let root_prepaint_start = self.prepaint_index();
            let root = if let Some(kept) = &kept_root {
                self.reuse_root_prepaint(kept, cx);
                None
            } else {
                let mut root = self.root.as_ref().unwrap().clone().into_any_element();
                let id = root.request_layout(self, cx);
                self.layout_engine
                    .as_mut()
                    .unwrap()
                    .stretch_auto_size_to_fill(id, root_size, scale_factor);
                root.prepaint_as_root(Point::default(), root_size.into(), self, cx);
                Some(root)
            };
            let root_prepaint = root_prepaint_start..self.prepaint_index();
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
            // Past the limit the attempt is the frame, and the inputs it
            // changed settle in the next one.
            if self.layout_retry && retries == MAX_LAYOUT_RETRIES {
                crate::dirty_sources::note("layout-retry-limit", std::panic::Location::caller());
                self.on_next_frame(|window, _| window.refresh());
                self.layout_retry = false;
            }
            retries += 1;
            if !self.layout_retry {
                self.layout_inputs = None;
                let counter = if root.is_some() {
                    &crate::frame_perf::ROOT_LAYOUTS
                } else {
                    &crate::frame_perf::ISOLATED_FRAMES
                };
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return PrepaintRoots {
                    root_element: root,
                    root_prepaint,
                    kept_root,
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
