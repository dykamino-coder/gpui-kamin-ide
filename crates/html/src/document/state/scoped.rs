//! Обёртка-элемент: держит состояние документа живым и делает его текущим
//! на фазах GPUI своего поддерева.
//!
//! Сборка кадра (`render`) возвращает элементы, а раскладка, подготовка и
//! краска идут позже, уже вне области сборки: размещение по якорю
//! (`AnchorPlace`) и замеры на подготовке читают правила документа именно
//! там. Узлом раскладки остаётся сам ребёнок — обёртка геометрию не меняет.

use super::DocState;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use std::rc::Rc;

pub struct Scoped {
    state: Rc<DocState>,
    child: AnyElement,
}

impl Scoped {
    /// Обернуть элемент документа `state`.
    pub fn wrap(state: &Rc<DocState>, child: AnyElement) -> AnyElement {
        Scoped {
            state: state.clone(),
            child,
        }
        .into_any_element()
    }
}

impl Element for Scoped {
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
        let _scope = self.state.enter_frame();
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
        let _scope = self.state.enter_frame();
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
        let _scope = self.state.enter_frame();
        self.child.paint(window, cx);
        self.state.painted.set(true);
    }
}

impl IntoElement for Scoped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
