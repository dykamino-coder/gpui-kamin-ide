//! Неподвижная обёртка внутри scrollport: компенсирует смещение прокрутки и красит ребёнка в
//! обрезке декораций, потому что рамки scrollport не принадлежат его прокручиваемому
//! содержимому (CSS Overflow 3 §2).

use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, ContentMask, Pixels, ScrollHandle, Window};
use std::{cell::Cell, rc::Rc};

pub(super) fn wrap(
    child: AnyElement,
    handle: ScrollHandle,
    clip: Rc<Cell<Option<ContentMask<Pixels>>>>,
) -> AnyElement {
    Stationary {
        child,
        handle,
        clip,
        token: Rc::new(()),
    }
    .hooked()
}
struct Stationary {
    child: AnyElement,
    handle: ScrollHandle,
    clip: Rc<Cell<Option<ContentMask<Pixels>>>>,
    /// Identity of the wrapper's scroll anchor for this build.
    token: Rc<()>,
}
impl Wrap for Stationary {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        // A scroll anchor: a replay that moves the scroller's node cancels the
        // move for the decorations, as the exact offset below does here (a
        // fixed box hangs on the viewport the same way, `ViewportAnchor`).
        let handle = self.handle.clone();
        let counter: Rc<dyn Fn() -> gpui::Point<Pixels>> = Rc::new(move || -handle.offset());
        let id = gpui::ScrollNodeId::of_token(&self.token);
        let (child, shift) = (&mut self.child, -self.handle.offset());
        window.with_scroll_anchor_prepaint(id, counter, |window| {
            window.with_content_mask_replaced(clip, |window| {
                window.with_exact_element_offset(shift, |window| child.prepaint(window, cx));
            });
        });
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        let (child, id) = (&mut self.child, gpui::ScrollNodeId::of_token(&self.token));
        window.with_scroll_node_paint(id, |window| {
            window.with_content_mask_replaced(clip, |window| child.paint(window, cx));
        });
    }
}
