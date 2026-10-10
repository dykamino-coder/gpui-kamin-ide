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

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Pixels, Window,
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
}

impl Transition {
    pub fn new(id: ElementId, seconds: f32, build: Rc<dyn Fn(f32) -> AnyElement>) -> Self {
        Transition {
            id,
            seconds: seconds.max(0.01),
            build,
        }
    }
}

impl Element for Transition {
    /// Собранное поддерево этого кадра.
    type RequestLayoutState = AnyElement;
    /// Область попадания курсора — по ней узнаётся наведение.
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let Some(global_id) = id else {
            // Без идентификатора памяти нет — рисуем обычный стиль.
            let mut el = (self.build)(0.0);
            let layout_id = el.request_layout(window, cx);
            return (layout_id, el);
        };
        let seconds = self.seconds;
        let build = self.build.clone();
        window.with_element_state::<State, _>(global_id, |state, window| {
            let mut st = state.unwrap_or_default();
            let now = Instant::now();
            let dt = st
                .last
                .map(|t| now.duration_since(t).as_secs_f32())
                .unwrap_or(0.0)
                // Потолок шага: после сворачивания окна между кадрами проходят
                // минуты, и переход обязан доиграть, а не прыгнуть.
                .min(0.25);
            st.last = Some(now);

            let target = if st.hovered { 1.0 } else { 0.0 };
            let step = dt / seconds;
            if (target - st.progress).abs() <= step {
                st.progress = target;
            } else {
                st.progress += step * (target - st.progress).signum();
            }
            if st.progress != target {
                window.request_animation_frame();
            }

            let mut el = build(st.progress);
            let layout_id = el.request_layout(window, cx);
            ((layout_id, el), st)
        })
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
        child.prepaint(window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
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
        child.paint(window, cx);
        let hovered = hitbox.is_hovered(window);
        let Some(global_id) = id else { return };
        // Наведение известно только после расстановки областей попадания,
        // поэтому цель перехода записывается здесь, а двигается на следующем
        // кадре — ровно как в браузере, где переход начинается с события.
        window.with_element_state::<State, _>(global_id, |state, window| {
            let mut st = state.unwrap_or_default();
            if st.hovered != hovered {
                st.hovered = hovered;
                window.request_animation_frame();
            }
            ((), st)
        });
    }
}

impl IntoElement for Transition {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
