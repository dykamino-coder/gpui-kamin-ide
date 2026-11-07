//! `transition` — плавный переход стиля при наведении.
//!
//! Почему отдельным элементом. Наведение в GPUI — это подмена стиля, она
//! мгновенная: у стиля нет промежуточных состояний, а у анимации нет понятия
//! «состояние изменилось». Плавность требует памяти между кадрами — где мы
//! сейчас между «обычным» и «наведённым». Такая память в GPUI есть ровно одна:
//! состояние элемента, доступное только своей реализации `Element`.
//!
//! Как устроено: элемент хранит долю перехода и время прошлого кадра, на
//! каждом кадре двигает долю к цели (наведён — к единице, нет — к нулю) и
//! просит следующий кадр, пока цель не достигнута. Поддерево пересобирается
//! замыканием по этой доле — дерево у нас и так собирается заново каждый кадр,
//! поэтому лишней работы это не создаёт; для узлов без перехода не создаётся
//! вообще ничего.

#[path = "transition/transition_traversal.rs"]
mod transition_traversal;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};
use std::rc::Rc;
use std::time::Instant;

/// Память между кадрами.
#[derive(Default)]
struct State {
    /// Где мы между обычным (0) и наведённым (1) стилем.
    progress: f32,
    /// Было ли наведение на прошлом кадре.
    hovered: bool,
    last: Option<Instant>,
}

/// Переход: пересобирает поддерево по доле перехода.
pub struct Transition {
    id: ElementId,
    seconds: f32,
    build: Rc<dyn Fn(f32) -> AnyElement>,
    doc: Rc<crate::document::state::DocState>,
}

impl Transition {
    pub fn new(
        id: ElementId,
        seconds: f32,
        build: Rc<dyn Fn(f32) -> AnyElement>,
        doc: Rc<crate::document::state::DocState>,
    ) -> Self {
        Transition {
            id,
            seconds: seconds.max(0.01),
            build,
            doc,
        }
    }
}

impl Element for Transition {
    /// Собранное поддерево этого кадра.
    type RequestLayoutState = AnyElement;
    /// Область попадания курсора — по ней узнаётся наведение.
    type PrepaintState = Hitbox;

    crate::hooks::keyed_element_identity!();

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        reblessive::Stack::new()
            .enter(|stack| self.request_layout_async(id, _inspector_id, window, cx, stack))
            .finish()
    }
    fn request_layout_async<'a>(
        &'a mut self,
        id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = (LayoutId, AnyElement)> + 'a {
        self.request_layout_on_stack(id, _inspector_id, window, cx, stack)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        reblessive::Stack::new()
            .enter(|stack| {
                self.prepaint_async(_id, _inspector_id, bounds, child, window, cx, stack)
            })
            .finish()
    }
    fn prepaint_async<'a>(
        &'a mut self,
        _id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &'a mut AnyElement,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = Hitbox> + 'a {
        self.prepaint_on_stack(bounds, child, window, cx, stack)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        reblessive::Stack::new()
            .enter(|stack| {
                self.paint_async(id, _inspector_id, _bounds, child, hitbox, window, cx, stack)
            })
            .finish();
    }

    fn paint_async<'a>(
        &'a mut self,
        id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &'a mut AnyElement,
        hitbox: &'a mut Hitbox,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            child.paint_async(window, cx, stack).await;
            let hovered = hitbox.is_hovered(window);
            let Some(global_id) = id else { return };
            // Наведение известно только после расстановки областей попадания,
            // поэтому цель перехода записывается здесь, а двигается на следующем
            // кадре — ровно как в браузере, где переход начинается с события.
            window.with_element_state::<State, _>(global_id, |state, window| {
                let mut st = state.unwrap_or_default();
                if st.hovered != hovered {
                    self.doc.values.defer_frame();
                    st.hovered = hovered;
                    window.request_animation_frame();
                }
                ((), st)
            });
        }
    }
}

impl IntoElement for Transition {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
