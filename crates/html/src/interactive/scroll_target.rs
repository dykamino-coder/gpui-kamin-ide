//! Scroll target: scroll the CSS box itself so an anonymous viewport cannot replace its sizing.
mod stationary;
use crate::dom::Element;
use crate::style::computed::{Computed, Sides};
use gpui::{
    Div, ElementId, InteractiveElement, Interactivity, IntoElement, ParentElement, ScrollHandle,
    StatefulInteractiveElement, Styled,
};
use std::cell::RefCell;

struct Target {
    node: u64,
    handle: ScrollHandle,
    horizontal: bool,
    vertical: bool,
    margin: Sides,
    applied: bool,
}
thread_local! { static TARGET: RefCell<Option<Target>> = const { RefCell::new(None) }; }
struct Restore(Option<Target>);
pub(crate) fn stationary(node: u64, child: gpui::AnyElement) -> gpui::AnyElement {
    let handle = TARGET.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|t| t.applied && t.node == node)
            .map(|t| t.handle.clone())
    });
    match (
        handle,
        crate::interactive::scroll_extent::decoration_clip(node),
    ) {
        (Some(handle), Some(clip)) => stationary::wrap(child, handle, clip),
        _ => child,
    }
}
// CSS Backgrounds 3 §2.11: the canvas covers the viewport, including gutters.
pub(crate) fn canvas(child: gpui::AnyElement) -> gpui::AnyElement {
    let Some((gutter, clip)) = super::scroll_extent::viewport_decoration() else {
        return child;
    };
    let Some(handle) = TARGET.with(|slot| slot.borrow().as_ref().map(|t| t.handle.clone())) else {
        return child;
    };
    let layer = gpui::div()
        .absolute()
        .top(gpui::px(-gutter[1]))
        .left(gpui::px(-gutter[0]))
        .right(gpui::px(-gutter[2]))
        .bottom(gpui::px(-gutter[3]))
        .child(child)
        .into_any_element();
    stationary::wrap(layer, handle, clip)
}

impl Drop for Restore {
    fn drop(&mut self) {
        TARGET.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

pub(super) fn build<R>(
    node: u64,
    handle: &ScrollHandle,
    horizontal: bool,
    vertical: bool,
    margin: Sides,
    f: impl FnOnce() -> R,
) -> (R, bool) {
    let prior = TARGET.with(|slot| {
        slot.replace(Some(Target {
            node,
            handle: handle.clone(),
            horizontal,
            vertical,
            margin,
            applied: false,
        }))
    });
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
    fn interactivity(&mut self) -> &mut Interactivity {
        self.0.interactivity()
    }
}
impl StatefulInteractiveElement for NativeScrollBox {}

pub(crate) fn attach(mut div: Div, e: &Element, c: &Computed) -> Div {
    // The CSS box owns both sizing and scroll state. An anonymous viewport
    // loses percentage, intrinsic, baseline and flex/grid sizing constraints.
    let target = TARGET.with(|slot| {
        let mut slot = slot.borrow_mut();
        let target = slot.as_mut()?;
        if target.applied || target.node != e.node_id || target.node != c.self_node {
            return None;
        }
        target.applied = true;
        Some((
            target.handle.clone(),
            target.horizontal,
            target.vertical,
            target.margin,
        ))
    });
    let Some((handle, horizontal, vertical, margin)) = target else {
        return div;
    };
    div = crate::paint::apply::margins(div, &margin);
    div.style().sizing_keywords = Some(crate::paint::apply::intrinsic_size::keywords(c));
    div.interactivity().element_id = Some(ElementId::Integer(e.node_id + 1));
    let mut stateful = NativeScrollBox(div).track_scroll(&handle);
    if horizontal {
        stateful = stateful.overflow_x_scroll();
    }
    if vertical {
        stateful = stateful.overflow_y_scroll();
    }
    let mut div = stateful.0;
    let bars = super::scroll_extent::classic::reserve(e.node_id, c);
    div.style().scrollbar_width = Some(gpui::px(c.scroll.thickness()).into());
    // Taffy reserves a classic gutter for Scroll, while Hidden has the same
    // sizing semantics without a gutter. Auto starts without bars.
    if horizontal && !bars[0] {
        div.style().overflow.x = Some(gpui::Overflow::Hidden);
    }
    if vertical && !bars[1] {
        div.style().overflow.y = Some(gpui::Overflow::Hidden);
    }
    // Hidden boxes still support programmatic scrolling and snapping, but must
    // not register GPUI wheel scrolling (CSS Overflow 3 §3).
    if c.overflow_x == Some(crate::style::computed::Overflow::Hidden) {
        div.style().overflow.x = Some(gpui::Overflow::Hidden);
    }
    if c.overflow_y == Some(crate::style::computed::Overflow::Hidden) {
        div.style().overflow.y = Some(gpui::Overflow::Hidden);
    }
    if let Some(marker) = crate::interactive::scroll_extent::marker(e.node_id) {
        div = div.child(marker);
    }
    div
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;
    use gpui::{CssSizingKeyword, Overflow, div, px};

    #[test]
    fn native_scroll_target_preserves_css_box_and_restores_nested_scope() {
        let mut e = crate::layout::table::anon::anon_element("div", vec![]);
        e.node_id = 7;
        let c = Computed {
            self_node: 7,
            width: Some(Len::FitContent),
            ..Computed::default()
        };
        let mut margin = Sides::default();
        margin.left = Some(Len::Px(10.0));
        let handle = ScrollHandle::default();
        let (_, matched) = build(7, &handle, true, false, margin, || {
            let (_, child_matched) = build(8, &handle, false, true, Sides::default(), || {
                let mut d = attach(div(), &e, &c);
                assert_eq!(d.style().overflow.x, None);
            });
            assert!(!child_matched);
            let mut d = attach(div(), &e, &c);
            assert_eq!(gpui::Element::id(&d), Some(ElementId::Integer(8)));
            assert_eq!(d.style().overflow.x, Some(Overflow::Hidden));
            assert_eq!(
                d.style().sizing_keywords,
                Some([Some(CssSizingKeyword::FitContent), None])
            );
            assert_eq!(d.style().margin.left, Some(px(10.0).into()));
            let mut second = attach(div(), &e, &c);
            assert_eq!(second.style().overflow.x, None);
        });
        assert!(matched);
        let mut after = attach(div(), &e, &c);
        assert_eq!(after.style().overflow.x, None);
    }

    #[test]
    fn colored_numeric_scrollport_tracks_the_css_box() {
        let mut e = crate::layout::table::anon::anon_element("div", vec![]);
        e.node_id = 7;
        let mut c = Computed {
            self_node: 7,
            width: Some(Len::Px(100.0)),
            ..Computed::default()
        };
        c.apply_one("scrollbar-color", "red blue");
        let handle = ScrollHandle::default();
        let (_, matched) = build(7, &handle, false, true, Sides::default(), || {
            let mut d = attach(div(), &e, &c);
            assert_eq!(d.style().overflow.y, Some(Overflow::Hidden));
            assert_eq!(d.style().sizing_keywords, Some([None, None]));
        });
        assert!(matched);
        let mut d = attach(div(), &e, &c);
        assert_eq!(d.style().overflow.y, None);
    }
}
