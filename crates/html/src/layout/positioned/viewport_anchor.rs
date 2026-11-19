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
}

impl ViewportAnchor {
    pub(crate) fn wrap(child: AnyElement, scroll: ScrollHandle) -> AnyElement {
        Self { child, scroll }.hooked()
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
        window.with_exact_element_offset(shift, |window| child.prepaint(window, cx));
    }
}
