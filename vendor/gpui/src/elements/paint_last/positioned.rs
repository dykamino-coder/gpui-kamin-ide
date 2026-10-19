//! Positioned paint wrappers retain layout and join the nearest collector.

use super::*;

/// Ребёнок `Div`, который рисуется после потока.
pub struct PaintLast {
    child: Option<AnyElement>,
    key: u64,
}

impl PaintLast {
    /// Обернуть элемент: раскладка та же, краска — после потока.
    pub fn new(child: AnyElement) -> Self {
        PaintLast {
            child: Some(child),
            key: 0,
        }
    }

    /// Номер элемента в разметке: порядок краски внутри собирателя.
    pub fn key(mut self, key: u64) -> Self {
        self.key = key;
        self
    }
}

impl Element for PaintLast {
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
        let child = self.child.as_mut().expect("PaintLast: ребёнок на месте");
        (child.request_layout(window, cx), ())
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
        if let Some(child) = self.child.as_mut() {
            child.prepaint(window, cx);
        }
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
        if hoist_open() && FRAMES.with(|f| !f.borrow().is_empty()) {
            if let Some(el) = self.child.take() {
                let ctx = window.paint_ctx();
                let key = self.key;
                FRAMES.with(|f| {
                    if let Some(frame) = f.borrow_mut().last_mut() {
                        frame.positioned.push(Hoisted { key, el, ctx });
                    }
                });
            }
            return;
        }
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for PaintLast {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
