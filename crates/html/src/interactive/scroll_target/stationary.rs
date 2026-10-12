//! CSS scrollport borders do not belong to its scrolling content (Overflow 3 §2).
use gpui::{
    AnyElement, App, Bounds, ContentMask, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, ScrollHandle, Window,
};
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
    .into_any_element()
}
struct Stationary {
    child: AnyElement,
    handle: ScrollHandle,
    clip: Rc<Cell<Option<ContentMask<Pixels>>>>,
}
impl IntoElement for Stationary {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Stationary {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        window.with_content_mask_replaced(clip, |window| {
            window.with_exact_element_offset(-self.handle.offset(), |window| {
                self.child.prepaint(window, cx);
            });
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let clip = self.clip.get().unwrap_or_else(|| window.content_mask());
        window.with_content_mask_replaced(clip, |window| self.child.paint(window, cx));
    }
}
