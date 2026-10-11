//! Tap: обёртка ребёнка, сообщающая его итоговые границы раскладке полос.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use std::cell::Cell;
use std::rc::Rc;

/// Прозрачная обёртка, запоминающая `LayoutId` ребёнка: после пробной
/// раскладки каркаса по нему читается размер самого ребёнка, а не каркаса.
pub(super) struct Tap {
    pub(super) inner: AnyElement,
    pub(super) id: Rc<Cell<Option<LayoutId>>>,
}

impl Element for Tap {
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
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let id = self.inner.request_layout(window, cx);
        self.id.set(Some(id));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
    }
}

impl IntoElement for Tap {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
