//! Применение CSS-размеров и нативной прокрутки к уже построенной коробке.

use crate::dom::Element;
use crate::style::computed::BoxStyle;
use gpui::{
    Div, InteractiveElement, Interactivity, ParentElement, StatefulInteractiveElement, Styled,
};

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
    c: &BoxStyle,
    cx: &crate::layout::cx::LayoutCx,
) -> Div {
    let vcx = cx.vcx();
    // The CSS box owns both sizing and scroll state. An anonymous viewport
    // loses percentage, intrinsic, baseline and flex/grid sizing constraints.
    let Some(crate::interactive::scroll_target::AppliedTarget {
        handle,
        horizontal,
        vertical,
        margin,
    }) = cx
        .scroll
        .target
        .as_ref()
        .and_then(|target| target.claim(e.node_id, c.self_node))
    else {
        return div;
    };
    div = crate::paint::apply::margins(div, &margin, vcx);
    div.style().sizing_keywords = Some(crate::paint::apply::intrinsic_size::keywords(c));
    div.interactivity().element_id = Some(e.state_id("html-scroll-content", vcx));
    let mut stateful = NativeScrollBox(div).track_scroll(&handle);
    if horizontal {
        stateful = stateful.overflow_x_scroll();
    }
    if vertical {
        stateful = stateful.overflow_y_scroll();
    }
    let mut div = stateful.0;
    let bars =
        crate::interactive::scroll_extent::classic::reserve(cx.scroll.frame.as_ref(), e.node_id, c);
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
        crate::interactive::scroll_extent::marker(cx.scroll.frame.as_ref(), e.node_id)
    {
        div = div.child(marker);
    }
    div
}

#[cfg(test)]
mod tests;
