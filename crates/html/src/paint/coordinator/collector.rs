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

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
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
    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}
    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.open {
            if let Some(pass) = current(window) {
                self.state
                    .set(Some(pass.open_reserving(window, self.inline_jobs)));
            }
        } else if let Some(guard) = self.state.take() {
            guard.pass.drain(guard.depth(), window, cx);
        }
    }
}

impl IntoElement for PaintCollect {
    type Element = Hooked<Self>;
    fn into_element(self) -> Self::Element {
        self.into_hooked()
    }
}
