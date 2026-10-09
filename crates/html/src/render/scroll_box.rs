//! Scroll the CSS box itself so an anonymous viewport cannot replace its sizing.
use crate::{computed::{Computed, Sides}, dom::Element};
use gpui::{Div, ElementId, InteractiveElement, Interactivity, ScrollHandle,
    StatefulInteractiveElement, Styled};
use std::cell::RefCell;

struct Target {
    node: u64, handle: ScrollHandle, horizontal: bool, vertical: bool,
    margin: Sides, applied: bool,
}
thread_local! { static TARGET: RefCell<Option<Target>> = const { RefCell::new(None) }; }
struct Restore(Option<Target>);
impl Drop for Restore {
    fn drop(&mut self) { TARGET.with(|slot| *slot.borrow_mut() = self.0.take()); }
}

pub(crate) fn build<R>(node: u64, handle: &ScrollHandle, horizontal: bool, vertical: bool,
    margin: Sides, f: impl FnOnce() -> R) -> (R, bool) {
    let prior = TARGET.with(|slot| slot.replace(Some(Target {
        node, handle: handle.clone(), horizontal, vertical, margin, applied: false,
    })));
    let restore = Restore(prior);
    let out = f();
    let applied = TARGET.with(|slot| slot.borrow().as_ref().is_some_and(|t| t.applied));
    drop(restore);
    (out, applied)
}

// This builder exposes GPUI's stateful setters and returns the same Div.
// Its ElementId is installed before stateful behavior; no layout box is added.
struct NativeScrollBox(Div);
impl InteractiveElement for NativeScrollBox {
    fn interactivity(&mut self) -> &mut Interactivity { self.0.interactivity() }
}
impl StatefulInteractiveElement for NativeScrollBox {}

pub(super) fn attach(mut div: Div, e: &Element, c: &Computed) -> Div {
    // Only intrinsic CSS roots have a native single-box sizing contract here.
    // Other scrollers still depend on the existing percentage/flex viewport adapter.
    let keyword = |value| matches!(value, Some(crate::value::Len::MinContent |
        crate::value::Len::MaxContent | crate::value::Len::FitContent));
    if !keyword(c.width) && !keyword(c.height) { return div; }
    let target = TARGET.with(|slot| {
        let mut slot = slot.borrow_mut();
        let target = slot.as_mut()?;
        if target.applied || target.node != e.node_id || target.node != c.self_node { return None; }
        target.applied = true;
        Some((target.handle.clone(), target.horizontal, target.vertical, target.margin))
    });
    let Some((handle, horizontal, vertical, margin)) = target else { return div; };
    div = crate::apply::margins(div, &margin);
    div.style().sizing_keywords = Some(crate::apply::intrinsic_size::keywords(c));
    div.interactivity().element_id = Some(ElementId::Integer(e.node_id + 1));
    let mut stateful = NativeScrollBox(div).track_scroll(&handle);
    if horizontal { stateful = stateful.overflow_x_scroll(); }
    if vertical { stateful = stateful.overflow_y_scroll(); }
    stateful.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Len;
    use gpui::{CssSizingKeyword, Overflow, div, px};

    #[test]
    fn native_scroll_target_preserves_css_box_and_restores_nested_scope() {
        let mut e = super::super::anon_element("div", vec![]);
        e.node_id = 7;
        let c = Computed { self_node: 7, width: Some(Len::FitContent), ..Computed::default() };
        let mut margin = Sides::default(); margin.left = Some(Len::Px(10.0));
        let handle = ScrollHandle::default();
        let (_, matched) = build(7, &handle, true, false, margin, || {
            let mut numeric = attach(div(), &e, &Computed { width: Some(Len::Px(100.0)), ..c.clone() });
            assert_eq!(numeric.style().overflow.x, None);
            let (_, child_matched) = build(8, &handle, false, true, Sides::default(), || {
                let mut d = attach(div(), &e, &c);
                assert_eq!(d.style().overflow.x, None);
            });
            assert!(!child_matched);
            let mut d = attach(div(), &e, &c);
            assert_eq!(gpui::Element::id(&d), Some(ElementId::Integer(8)));
            assert_eq!(d.style().overflow.x, Some(Overflow::Scroll));
            assert_eq!(d.style().sizing_keywords, Some([Some(CssSizingKeyword::FitContent), None]));
            assert_eq!(d.style().margin.left, Some(px(10.0).into()));
            let mut second = attach(div(), &e, &c);
            assert_eq!(second.style().overflow.x, None);
        });
        assert!(matched);
        let mut after = attach(div(), &e, &c);
        assert_eq!(after.style().overflow.x, None);
    }
}
