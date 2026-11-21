//! A `position: fixed` box whose containing block is the viewport hangs on
//! the viewport, not on the scrolled document (css-position-3 §fixed-pos:
//! "the box does not move when the document is scrolled").
//!
//! The box is laid out and painted where it was (the root ICB layer inside
//! the root scrollport), so its geometry and paint order stay those of the
//! layer. Only its spatial parent changes: prepaint cancels the root scroll
//! offset, the same tool sticky uses for its shift. Blink hangs the fixed
//! context of the LayoutView on the transform before the scroll translation,
//! Servo gives fixed descendants the root reference frame instead of the
//! root scroll frame; both move only the scroll node when the page scrolls.
//! A box with a fixed containing block (transform, filter, contain, ...)
//! never gets here: its layer is that ancestor's, and it scrolls with it.
//!
//! The offset is read from the root scroll handle, the one record of the
//! root offset (`DocState::root_scroll` in the document view, otherwise the
//! scroll frame of `html-viewport`): hit testing and bounds that prepaint
//! records follow it, so they report viewport coordinates.

use crate::hooks::{IntoHooked, Wrap};
use gpui::{
    AnyElement, App, Bounds, IntoElement, ParentElement, Pixels, ScrollHandle, Styled, Window,
};

/// The box a viewport-fixed box is laid out in: absolutely positioned with
/// zero insets, it is exactly the root scrollport's padding box, the same
/// containing block the box had as a direct child of the scrollport. The
/// scrollport measures its content from its direct children, so the fixed
/// box no longer extends the root scroll range (Blink: `LayoutView` keeps
/// fixed descendants out of its scrollable overflow).
pub(crate) fn viewport_box(child: AnyElement) -> AnyElement {
    gpui::div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .child(child)
        .into_any_element()
}

/// Build a viewport-fixed box's subtree outside the root scroll frame: the
/// box is not part of the scrolled content (nchor_of). The one place the
/// build context drops the frame for it.
pub(crate) fn unscroll(cx: &mut crate::layout::cx::LayoutCx, anchored: bool) {
    if anchored {
        cx.scroll.frame = None;
        // The anchor cancels the root offset: its content prepaints in
        // viewport space, under no scroll node.
        cx.scroll.node = None;
    }
}

/// The root scroll handle a fixed box hangs on: `Some` when the box's
/// containing block is the viewport (`viewport_fixed`: `position: fixed`
/// without a fixed containing block among its ancestors) and the open ICB
/// layer is a root scrollport's. Such a box is not part of the scrolled
/// content and adds nothing to the root's scrollable overflow, so its
/// subtree is built without the root scroll frame.
pub(crate) fn anchor_of(
    viewport_fixed: bool,
    positioning: &crate::layout::positioned::containing_block::Positioning,
) -> Option<ScrollHandle> {
    viewport_fixed
        .then(|| positioning.viewport_anchor())
        .flatten()
}

/// `child` hung on `anchor`, or `child` itself.
pub(crate) fn anchored(child: AnyElement, anchor: Option<&ScrollHandle>) -> AnyElement {
    match anchor {
        Some(scroll) => ViewportAnchor::wrap(child, scroll.clone()),
        None => child,
    }
}

pub(crate) struct ViewportAnchor {
    child: AnyElement,
    scroll: ScrollHandle,
    /// Identity of the anchor's scroll node for this build.
    token: std::rc::Rc<()>,
}

impl ViewportAnchor {
    pub(crate) fn wrap(child: AnyElement, scroll: ScrollHandle) -> AnyElement {
        Self {
            child,
            scroll,
            token: std::rc::Rc::new(()),
        }
        .hooked()
    }
}

impl Wrap for ViewportAnchor {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        // GPUI scroll offsets are non-positive: content moves up and left.
        let shift = -self.scroll.offset();
        let child = &mut self.child;
        // A scroll anchor: a replay that moves the root scroll node cancels
        // the move for the box as this shift does (`with_scroll_anchor_prepaint`).
        let scroll = self.scroll.clone();
        let counter: std::rc::Rc<dyn Fn() -> gpui::Point<Pixels>> =
            std::rc::Rc::new(move || -scroll.offset());
        let id = gpui::ScrollNodeId::of_token(&self.token);
        window.with_scroll_anchor_prepaint(id, counter, |window| {
            window.with_exact_element_offset(shift, |window| child.prepaint(window, cx))
        });
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let id = gpui::ScrollNodeId::of_token(&self.token);
        let child = &mut self.child;
        window.with_scroll_node_paint(id, |window| child.paint(window, cx));
    }
}
