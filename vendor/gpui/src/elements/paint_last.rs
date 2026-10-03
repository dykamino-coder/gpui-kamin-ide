// KaminIDE patch: краска ребёнка ПОСЛЕ его братьев без смены раскладки.
//
// Позиционированный элемент с `z-index: auto | 0` рисуется на шаге 8
// приложения E CSS 2.1 — после всего обычного содержимого потока, — а место
// в раскладке держит своё (относительный сдвиг, абсолют с одной свободной
// осью). Порядок краски в GPUI — порядок детей, и переставить ребёнка
// значило бы сдвинуть раскладку. Обёртка ничего не меняет в раскладке и
// prepaint (узел раскладки — узел ребёнка), а `Div::paint` рисует такие
// обёртки вторым проходом, в их порядке среди детей. Отложенная краска
// (`deferred`) тут не годится: она выносит поддерево из масок и вложенной
// не бывает.
use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};

/// Ребёнок `Div`, который рисуется после всех обычных братьев.
pub struct PaintLast {
    child: AnyElement,
}

impl PaintLast {
    /// Обернуть элемент: раскладка та же, краска — вторым проходом родителя.
    pub fn new(child: AnyElement) -> Self {
        PaintLast { child }
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
        (self.child.request_layout(window, cx), ())
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

impl IntoElement for PaintLast {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
