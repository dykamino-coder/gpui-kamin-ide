//! Контекст хозяина сохраняет layout identity и метаданные ребёнка.

use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use std::any::Any;
use std::rc::Rc;

/// Фаза, на которую обёртка входит в контекст.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopePhase {
    /// Запрос раскладки дочернего элемента.
    RequestLayout,
    /// Предварительная отрисовка (prepaint).
    Prepaint,
    /// Отрисовка.
    Paint,
}

/// Вход в контекст хозяина: guard живёт до конца фазы.
pub type ScopeEnter = Rc<dyn Fn(ScopePhase) -> Box<dyn Any>>;

/// Обёртка `child` с контекстом `enter`.
pub struct ContextScope {
    enter: ScopeEnter,
    child: AnyElement,
}

impl ContextScope {
    /// Оборачивает `child`: каждая фаза выполняется внутри `enter(phase)`.
    pub fn new(enter: ScopeEnter, child: AnyElement) -> Self {
        ContextScope { enter, child }
    }
}

impl Element for ContextScope {
    fn paint_boundary(&self) -> bool { false }
    fn paint_metadata(&self) -> Option<&dyn Any> { self.child.paint_metadata() }

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
        let _scope = (self.enter)(ScopePhase::RequestLayout);
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
        let _scope = (self.enter)(ScopePhase::Prepaint);
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
        let _scope = (self.enter)(ScopePhase::Paint);
        self.child.paint(window, cx);
    }
}

impl IntoElement for ContextScope {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
