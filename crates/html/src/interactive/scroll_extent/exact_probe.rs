//! Scrollbar decisions use layout geometry before device snapping (CSS Overflow 3 §2.2).
//! Independent rounding of a 10px child and a 25px scrollport can fabricate overflow.

use crate::hooks::Hooks;
use gpui::{AnyElement, App, Bounds, IntoElement, LayoutId, Pixels, Styled, Window, px};

thread_local! {
    static STICKY_SHIFT: std::cell::Cell<gpui::Point<Pixels>> = const { std::cell::Cell::new(gpui::point(px(0.0), px(0.0))) };
}
pub(super) fn sticky_shift() -> gpui::Point<Pixels> {
    STICKY_SHIFT.with(std::cell::Cell::get)
}
struct RestoreShift(gpui::Point<Pixels>);
impl Drop for RestoreShift {
    fn drop(&mut self) {
        STICKY_SHIFT.with(|s| s.set(self.0));
    }
}
pub(crate) fn sticky_geometry<R>(shift: gpui::Point<Pixels>, f: impl FnOnce() -> R) -> R {
    let prior = STICKY_SHIFT.with(|s| s.replace(s.get() + shift));
    let restore = RestoreShift(prior);
    let result = f();
    drop(restore);
    result
}

pub(super) struct ExactProbe {
    child: AnyElement,
    measure: Box<dyn FnMut(Bounds<Pixels>, Bounds<Pixels>)>,
}
impl ExactProbe {
    pub(super) fn new(measure: impl FnMut(Bounds<Pixels>, Bounds<Pixels>) + 'static) -> Self {
        Self {
            child: gpui::div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .into_any_element(),
            measure: Box::new(measure),
        }
    }
}
impl Hooks for ExactProbe {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let id = self.child.request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        id: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let exact = Bounds {
            origin: window.layout_origin_unrounded(*id),
            size: window.layout_size_unrounded(*id),
        };
        let half = px(0.5 / window.scale_factor().max(0.01) + 1e-4);
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= half;
        // A relocated element has a different containing block; retain its
        // prepaint position, while still using its unrounded extent.
        let exact = if near(exact.left(), bounds.left())
            && near(exact.top(), bounds.top())
            && near(exact.right(), bounds.right())
            && near(exact.bottom(), bounds.bottom())
        {
            exact
        } else {
            bounds
        };
        let mut normal = exact;
        normal.origin -= STICKY_SHIFT.with(std::cell::Cell::get);
        (self.measure)(normal, bounds);
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut Self::Layout,
        _: &mut Self::Prepaint,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}
