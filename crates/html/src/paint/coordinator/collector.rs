//! Парные метки CSS stacking context с прежними нулевыми layout-коробками.

use super::*;
use crate::hooks::{Hooked, Hooks, IntoHooked};
use gpui::{App, Bounds, IntoElement, LayoutId, Pixels, Position, Style};

pub(crate) struct PaintCollect {
    open: bool,
    /// Строковых заданий, собранных сборкой между метками (`Deferred::inline_jobs`).
    inline_jobs: u32,
    state: Rc<Cell<Option<FrameGuard>>>,
}

impl PaintCollect {
    pub(crate) fn pair() -> (Self, Self) {
        Self::pair_reserving(0)
    }

    /// Пара, чей кадр сразу берёт место под `inline_jobs` строковых заданий.
    pub(crate) fn pair_reserving(inline_jobs: u32) -> (Self, Self) {
        let state = Rc::default();
        (
            Self {
                open: true,
                inline_jobs,
                state: Rc::clone(&state),
            },
            Self {
                open: false,
                inline_jobs: 0,
                state,
            },
        )
    }
}

impl Hooks for PaintCollect {
    type Layout = ();
    type Prepaint = ();
    fn paint_boundary(&self) -> bool {
        false
    }

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Default::default()
                },
                [],
                cx,
            ),
            (),
        )
    }
    async fn prepaint_async(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
        _stack: &mut gpui::PaintStack,
    ) {
    }
    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut (),
        prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, state, prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _: Bounds<Pixels>,
        _: &'a mut (),
        _: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            if self.open {
                if let Some(pass) = current(window) {
                    self.state
                        .set(Some(pass.open_reserving(window, self.inline_jobs)));
                }
            } else if let Some(guard) = self.state.take() {
                // Отложенный ребёнок может открыть следующий collector: тот же
                // планировщик сохраняет кадр до завершения всей его очереди.
                guard
                    .pass
                    .drain_async(guard.depth(), window, cx, stack)
                    .await;
            }
        }
    }
}

impl IntoElement for PaintCollect {
    type Element = Hooked<Self>;
    fn into_element(self) -> Self::Element {
        self.into_hooked()
    }
}
