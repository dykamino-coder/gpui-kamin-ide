//! Scroll target: scroll the CSS box itself so an anonymous viewport cannot replace its sizing.

mod stationary;
use crate::dom::Element;
use crate::style::computed::{Computed, Sides};
use gpui::{
    Div, ElementId, InteractiveElement, Interactivity, IntoElement, ParentElement, ScrollHandle,
    StatefulInteractiveElement, Styled,
};
use std::cell::Cell;
use std::rc::Rc;

pub(crate) struct Target {
    node: u64,
    handle: ScrollHandle,
    horizontal: bool,
    vertical: bool,
    margin: Sides,
    applied: Cell<bool>,
}
pub(crate) fn stationary(
    frame: Option<&super::scroll_extent::Frame>,
    target: Option<&Target>,
    node: u64,
    child: gpui::AnyElement,
) -> gpui::AnyElement {
    let handle = target
        .filter(|t| t.applied.get() && t.node == node)
        .map(|t| t.handle.clone());
    match (
        handle,
        crate::interactive::scroll_extent::decoration_clip(frame, node),
    ) {
        (Some(handle), Some(clip)) => stationary::wrap(child, handle, clip),
        _ => child,
    }
}
// CSS Backgrounds 3 §2.11: the canvas covers the viewport, including gutters.
pub(crate) fn canvas(
    frame: Option<&super::scroll_extent::Frame>,
    target: Option<&Target>,
    child: gpui::AnyElement,
) -> gpui::AnyElement {
    let Some((gutter, clip)) = super::scroll_extent::viewport_decoration(frame) else {
        return child;
    };
    let Some(handle) = target.map(|t| t.handle.clone()) else {
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

impl Target {
    pub(super) fn new(
        node: u64,
        handle: &ScrollHandle,
        horizontal: bool,
        vertical: bool,
        margin: Sides,
    ) -> Rc<Self> {
        Rc::new(Self {
            node,
            handle: handle.clone(),
            horizontal,
            vertical,
            margin,
            applied: Cell::new(false),
        })
    }

    pub(super) fn applied(&self) -> bool {
        self.applied.get()
    }
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

pub(crate) fn attach(
    mut div: Div,
    e: &Element,
    c: &Computed,
    cx: &crate::layout::cx::LayoutCx,
) -> Div {
    let vcx = cx.vcx();
    // The CSS box owns both sizing and scroll state. An anonymous viewport
    // loses percentage, intrinsic, baseline and flex/grid sizing constraints.
    let Some(target) = cx.scroll_target.as_ref().filter(|target| {
        !target.applied.get() && target.node == e.node_id && target.node == c.self_node
    }) else {
        return div;
    };
    target.applied.set(true);
    let (handle, horizontal, vertical, margin) = (
        target.handle.clone(),
        target.horizontal,
        target.vertical,
        target.margin,
    );
    div = crate::paint::apply::margins(div, &margin, vcx);
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
    let bars = super::scroll_extent::classic::reserve(cx.scroll_frame.as_ref(), e.node_id, c);
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
    if let Some(marker) =
        crate::interactive::scroll_extent::marker(cx.scroll_frame.as_ref(), e.node_id)
    {
        div = div.child(marker);
    }
    div
}

#[cfg(test)]
mod tests;
