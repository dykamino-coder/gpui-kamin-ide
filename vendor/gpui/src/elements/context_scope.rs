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
    fn paint_boundary(&self) -> bool {
        false
    }
    fn paint_metadata(&self) -> Option<&dyn Any> {
        self.child.paint_metadata()
    }

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
        reblessive::Stack::new()
            .enter(|stack| self.request_layout_async(_id, _inspector_id, window, cx, stack))
            .finish()
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
        reblessive::Stack::new()
            .enter(|stack| {
                self.prepaint_async(_id, _inspector_id, _bounds, _state, window, cx, stack)
            })
            .finish();
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
        reblessive::Stack::new()
            .enter(|stack| {
                self.paint_async(
                    _id,
                    _inspector_id,
                    _bounds,
                    _state,
                    _prepaint,
                    window,
                    cx,
                    stack,
                )
            })
            .finish();
    }

    fn request_layout_async<'a>(
        &'a mut self,
        _: Option<&'a GlobalElementId>,
        _: Option<&'a InspectorElementId>,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut crate::PaintStack,
    ) -> impl std::future::Future<Output = (LayoutId, ())> + 'a {
        async move {
            let _scope = (self.enter)(ScopePhase::RequestLayout);
            (self.child.request_layout_async(window, cx, stack).await, ())
        }
    }

    fn prepaint_async<'a>(
        &'a mut self,
        _: Option<&'a GlobalElementId>,
        _: Option<&'a InspectorElementId>,
        _: Bounds<Pixels>,
        _: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut crate::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            let _scope = (self.enter)(ScopePhase::Prepaint);
            self.child.prepaint_async(window, cx, stack).await;
        }
    }

    fn paint_async<'a>(
        &'a mut self,
        _id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &'a mut (),
        _prepaint: &'a mut (),
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut crate::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            let _scope = (self.enter)(ScopePhase::Paint);
            self.child.paint_async(window, cx, stack).await;
        }
    }
}

impl IntoElement for ContextScope {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
