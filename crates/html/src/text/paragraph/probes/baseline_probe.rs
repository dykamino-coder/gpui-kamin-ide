//! Baseline probe for probes; split out to keep the owning module within 250 lines.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px, size,
};

/// Щуп базовой линии атома: пустой лист с базовой линией на своём верху.
/// В ряду `align-items: baseline` рядом с атомом его верх встаёт ровно на
/// базовую линию атома, и раскладка отдаёт её положением щупа. Для атома без
/// базовой линии taffy берёт нижний край полей (`flexbox.rs`) — у замещаемого
/// это и есть его базовая по §10.8.1.
pub(crate) struct BaselineProbe {
    pub(crate) slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
}

impl Element for BaselineProbe {
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
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let id = window
            .request_measured_layout_with_baselines(gpui::Style::default(), |_, _, _, _| {
                (size(px(0.), px(0.)), Some(px(0.)), Some(px(0.)))
            });
        self.slot.set(Some(id));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for BaselineProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Обёртка, запоминающая узел раскладки своего ребёнка: по нему берётся
/// ТОЧНЫЙ размер атома (`Window::layout_exact`) — округлённый к точке
/// устройства прибавлял до 0.4px на атом, и ряд атомов ровно в ширину строки
/// в неё уже не влезал (`c542-letter-sp-001-ref`, `c5505-mrgn-000`).
pub(crate) struct LayoutTap {
    pub(crate) child: AnyElement,
    pub(crate) slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
}

impl Element for LayoutTap {
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
        let id = self.child.request_layout(window, cx);
        self.slot.set(Some(id));
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

impl IntoElement for LayoutTap {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
