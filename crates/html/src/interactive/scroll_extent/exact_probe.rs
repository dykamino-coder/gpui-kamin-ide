//! Щуп неокруглённой геометрии для решений о полосах (CSS Overflow 3 §2.2): коробки берутся
//! до привязки к пикселям устройства, иначе раздельное округление ребёнка 10px и scrollport
//! 25px выдумывает переполнение. Здесь же сдвиги липких коробок и обращённой проекции
//! прокрутки, которые щупы вычитают.

use gpui::{Pixels, px};

/// A paint-time offset that a sticky box (css-position-3 §sticky-pos) or a
/// reversed scroll projection applies around its subtree in prepaint. Probes
/// measure layout geometry WITHOUT these offsets, so each probe keeps the
/// chain of the shifting boxes it was built inside (`ScrollContext::sticky`) and
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
