//! Scrollbar decisions use layout geometry before device snapping (CSS Overflow 3 §2.2).
//! Independent rounding of a 10px child and a 25px scrollport can fabricate overflow.

use crate::hooks::Hooks;
use gpui::{AnyElement, App, Bounds, IntoElement, LayoutId, Pixels, Styled, Window, px};

/// A paint-time offset that a sticky box (css-position-3 §sticky-pos) or a
/// reversed scroll projection applies around its subtree in prepaint. Probes
/// measure layout geometry WITHOUT these offsets, so each probe keeps the
/// chain of the shifting boxes it was built inside (`LayoutCx::sticky`) and
/// subtracts the shifts that are being applied while it prepaints.
pub(crate) struct ShiftLink {
    shift: std::cell::Cell<gpui::Point<Pixels>>,
    /// The shift is applied only while its box prepaints its child.
    active: std::cell::Cell<bool>,
    outer: Option<StickyShift>,
}
pub(crate) type StickyShift = std::rc::Rc<ShiftLink>;
impl ShiftLink {
    /// A link for a shifting box built inside the chain `outer`.
    pub(crate) fn inside(outer: Option<&StickyShift>) -> StickyShift {
        std::rc::Rc::new(ShiftLink {
            shift: std::cell::Cell::new(gpui::point(px(0.0), px(0.0))),
            active: std::cell::Cell::new(false),
            outer: outer.cloned(),
        })
    }
    /// Prepaint the subtree under `f` with this box's `shift` applied.
    pub(crate) fn applied<R>(&self, shift: gpui::Point<Pixels>, f: impl FnOnce() -> R) -> R {
        struct Inactive<'a>(&'a ShiftLink);
        impl Drop for Inactive<'_> {
            fn drop(&mut self) {
                self.0.active.set(false);
            }
        }
        self.shift.set(shift);
        self.active.set(true);
        let _inactive = Inactive(self);
        f()
    }
}
/// Sum of the shifts being applied along `link`'s chain.
pub(crate) fn sticky_shift(link: Option<&StickyShift>) -> gpui::Point<Pixels> {
    let mut sum = gpui::point(px(0.0), px(0.0));
    let mut at = link;
    while let Some(l) = at {
        if l.active.get() {
            sum += l.shift.get();
        }
        at = l.outer.as_ref();
    }
    sum
}

pub(super) struct ExactProbe {
    child: AnyElement,
    sticky: Option<StickyShift>,
    measure: Box<dyn FnMut(Bounds<Pixels>, Bounds<Pixels>)>,
}
impl ExactProbe {
    pub(super) fn new(
        sticky: Option<StickyShift>,
        measure: impl FnMut(Bounds<Pixels>, Bounds<Pixels>) + 'static,
    ) -> Self {
        Self {
            sticky,
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
        normal.origin -= sticky_shift(self.sticky.as_ref());
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
