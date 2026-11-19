//! Периодически обновляемая часть окна, которая перерисовывается одна
//! (INC-2026-0064).
//!
//! Подпись времени работы, спиннер, мигание каретки меняются по такту
//! `anim_tick`. Раньше каждый шаг уведомлял корень окна: секундная подпись
//! стоила раскладки всего окна (~760 узлов) раз в секунду. Здесь содержимое
//! рисует собственный вью в коробке, которую задаёт раскладка родителя
//! (`gpui::isolated`): шаг уведомляет только этот вью, корень и соседи берутся
//! из прошлого кадра. Коробка не зависит от содержимого — размер задаёт стиль
//! элемента или `sizer` (шаблон самого широкого содержимого). Содержимое
//! рисуется поверх дерева родителя с его клипом: годится для мелких листьев,
//! которые ничто не перекрывает.

use gpui::AppContext as _;
use gpui::{
    AnyElement, App, Bounds, Context, Element, ElementId, Entity, GlobalElementId,
    InspectorElementId, IntoElement, Isolated, LayoutId, Pixels, Render, StyleRefinement, Styled,
    Window,
};
use std::rc::Rc;

type Draw = Rc<dyn Fn(u64) -> AnyElement>;

/// Вью ступенчатой анимации: номер шага по общему такту → содержимое.
struct Tick {
    step_ms: u64,
    draw: Draw,
}

impl Render for Tick {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let n = crate::ui::anim_tick::step_for_view(cx.entity_id(), self.step_ms);
        (self.draw)(n)
    }
}

/// Часть окна `id`, которая каждые `step_ms` перерисовывается одна:
/// `draw(номер шага)` строит содержимое.
pub(crate) fn ticking(
    id: impl Into<ElementId>,
    step_ms: u64,
    draw: impl Fn(u64) -> AnyElement + 'static,
) -> Ticking {
    Ticking {
        id: id.into(),
        step_ms,
        draw: Rc::new(draw),
        style: StyleRefinement::default(),
        sizer: None,
    }
}

/// См. [`ticking`].
pub(crate) struct Ticking {
    id: ElementId,
    step_ms: u64,
    draw: Draw,
    style: StyleRefinement,
    sizer: Option<AnyElement>,
}

impl Ticking {
    /// Коробка — по размеру `sizer` (раскладывается, не рисуется).
    pub(crate) fn sizer(mut self, sizer: impl IntoElement) -> Self {
        self.sizer = Some(sizer.into_any_element());
        self
    }
}

impl Styled for Ticking {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Element for Ticking {
    type RequestLayoutState = Isolated;
    type PrepaintState = ();

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
    ) -> (LayoutId, Isolated) {
        let (step_ms, draw) = (self.step_ms, self.draw.clone());
        // Своё хранение вью, а не `use_keyed_state`: тот подписывает родителя
        // на уведомления состояния, и шаг снова перерисовывал бы корень.
        let view = window.with_element_state(id.expect("ticking has an id"), |state, _| {
            let view: Entity<Tick> = match state {
                Some(view) => view,
                None => cx.new(|_| Tick {
                    step_ms,
                    draw: draw.clone(),
                }),
            };
            (view.clone(), view)
        });
        // Новое содержимое родителя — без уведомления: родитель и так рисует
        // вью заново в этом кадре.
        view.update(cx, |tick, _| {
            tick.step_ms = step_ms;
            tick.draw = draw;
        });
        let mut inner = gpui::isolated(view);
        *inner.style() = self.style.clone();
        if let Some(sizer) = self.sizer.take() {
            inner = inner.sizer(sizer);
        }
        let (layout_id, ()) = inner.request_layout(None, None, window, cx);
        (layout_id, inner)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        inner: &mut Isolated,
        window: &mut Window,
        cx: &mut App,
    ) {
        inner.prepaint(None, None, bounds, &mut (), window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _inner: &mut Isolated,
        _prepaint: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for Ticking {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
