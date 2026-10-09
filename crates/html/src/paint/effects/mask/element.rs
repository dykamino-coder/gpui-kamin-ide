//! Элементы MaskKeep и MaskUse: снимок и применение маски.

use crate::interact::*;

/// Маска обрезающего предка для ОТЛОЖЕННОГО слоя (`z-index > 0`).
///
/// GPUI рисует отложенные элементы после всего дерева и восстанавливает им
/// стек id и стилей, но не маску содержимого: `overflow: hidden/clip`
/// предка их не режет (`overflow-clip-margin-011..022`). Наружная обёртка
/// стоит в потоке и на `prepaint` запоминает текущую маску, внутренняя —
/// внутри `deferred` — рисует ребёнка под ней. Фиксированному слою маска
/// не нужна: его содержащий блок — окно.
pub type MaskCell = std::rc::Rc<std::cell::RefCell<Option<gpui::ContentMask<Pixels>>>>;

pub struct MaskKeep {
    pub cell: MaskCell,
    pub child: AnyElement,
}

impl IntoElement for MaskKeep {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for MaskKeep {
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
        *self.cell.borrow_mut() = Some(window.content_mask());
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

pub struct MaskUse {
    pub cell: MaskCell,
    pub child: AnyElement,
}

impl IntoElement for MaskUse {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for MaskUse {
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
        let mask = self.cell.borrow().clone();
        window.with_content_mask(mask, |window| self.child.prepaint(window, cx));
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
        let mask = self.cell.borrow().clone();
        window.with_content_mask(mask, |window| self.child.paint(window, cx));
    }
}
