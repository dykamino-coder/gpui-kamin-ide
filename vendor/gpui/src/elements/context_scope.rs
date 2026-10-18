// KaminIDE patch: прозрачная обёртка, которая ставит контекст хозяина на
// фазы своего поддерева.
//
// Документ движка HTML держит свои правила и арены в состоянии документа и
// делает его текущим на время работы с ним; раскладка, подготовка и краска
// идут после сборки кадра, поэтому обёртка входит в контекст на каждой фазе
// (`enter` возвращает guard, его сброс — выход). Для раскладки и порядка
// краски обёртки нет: узел раскладки — узел ребёнка, собиратель `PaintLast`
// проходит её насквозь (`AnyElement::paint`), второй проход `Div` видит
// `PaintLast` внутри неё (`is_paint_last`).

use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use std::any::Any;
use std::rc::Rc;

/// Фаза, на которую обёртка входит в контекст.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopePhase {
    RequestLayout,
    Prepaint,
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
    pub fn new(enter: ScopeEnter, child: AnyElement) -> Self {
        ContextScope { enter, child }
    }
}

/// `PaintLast` — сам элемент или внутри прозрачных обёрток.
pub(crate) fn is_paint_last(el: &mut AnyElement) -> bool {
    if el.downcast_mut::<crate::PaintLast>().is_some() {
        return true;
    }
    el.downcast_mut::<ContextScope>()
        .is_some_and(|scope| is_paint_last(&mut scope.child))
}

impl Element for ContextScope {
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
