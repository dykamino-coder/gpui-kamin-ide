//! Парные метки CSS stacking context с прежними нулевыми layout-коробками.

use super::*;
use crate::hooks::{Hooked, Hooks, IntoHooked};
use gpui::{App, Bounds, IntoElement, LayoutId, Pixels, Position, Style};

pub(crate) struct PaintCollect {
    open: bool,
    state: Rc<Cell<Option<FrameGuard>>>,
}

impl PaintCollect {
    pub(crate) fn pair() -> (Self, Self) {
        let state = Rc::default();
        (
            Self {
                open: true,
                state: Rc::clone(&state),
            },
            Self { open: false, state },
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
                self.state.set(Some(pass.open(window)));
            }
        } else if let Some(guard) = self.state.take() {
            guard.pass.drain(guard.depth, window, cx);
        }
    }
}

impl IntoElement for PaintCollect {
    type Element = Hooked<Self>;
    fn into_element(self) -> Self::Element {
        self.into_hooked()
    }
}
