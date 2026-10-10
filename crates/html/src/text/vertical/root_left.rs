//! Root left for vertical; split out to keep the owning module within 250 lines.

use super::ROOT_LEFT;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

/// Левый край коробки корня (см. `ROOT_LEFT`).
pub fn root_left_prev(key: u64) -> Option<f32> {
    ROOT_LEFT.with(|c| c.borrow().get(&key).copied())
}

/// Обёртка, которая на подготовке записывает левый край своей коробки минус
/// `offset` (поля тела и рамка/отбивка корня) в `ROOT_LEFT` — раскладку не
/// меняет: узел раскладки — сам ребёнок.
pub struct RecordRootLeft {
    pub(crate) child: AnyElement,
    pub(crate) key: u64,
    pub(crate) offset: f32,
}

pub fn record_root_left(child: AnyElement, key: u64, offset: f32) -> AnyElement {
    RecordRootLeft { child, key, offset }.into_any_element()
}

impl Element for RecordRootLeft {
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
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let left = f32::from(bounds.origin.x) - self.offset;
        ROOT_LEFT.with(|c| c.borrow_mut().insert(self.key, left));
        self.child.prepaint(window, cx);
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
        self.child.paint(window, cx);
    }
}

impl IntoElement for RecordRootLeft {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
