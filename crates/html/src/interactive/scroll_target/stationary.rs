//! CSS scrollport borders do not belong to its scrolling content (Overflow 3 §2).

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
    }
    .hooked()
}
struct Stationary {
    child: AnyElement,
    handle: ScrollHandle,
    clip: Rc<Cell<Option<ContentMask<Pixels>>>>,
}
impl Wrap for Stationary {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        window.with_content_mask_replaced(clip, |window| {
            window.with_exact_element_offset(-self.handle.offset(), |window| {
                self.child.prepaint(window, cx);
            });
        });
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        window.with_content_mask_replaced(clip, |window| self.child.paint(window, cx));
    }
}
