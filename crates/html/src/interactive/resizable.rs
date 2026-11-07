//! GPUI-элемент `Resizable` для CSS `resize`: хранит между кадрами заданный мышью размер и
//! состояние перетаскивания, ловит нажатия только в угловой ручке и перестраивает содержимое
//! с новыми `width`/`height`. Отдельный элемент нужен потому, что память между кадрами в
//! GPUI есть только у собственного `Element`.

#[path = "resizable/resizable_traversal.rs"]
mod resizable_traversal;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Window, px,
};
use std::rc::Rc;

/// По каким осям разрешено тянуть.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResizeAxis {
    Both,
    Horizontal,
    Vertical,
}

/// Память между кадрами: заданный пользователем размер и состояние перетаскивания.
#[derive(Default, Clone, Copy)]
struct State {
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
    pub(crate) dragging: bool,
    /// Размер на момент нажатия — от него считается сдвиг.
    pub(crate) from: (f32, f32),
    pub(crate) at: (f32, f32),
}

/// Сторона квадратной ручки в углу.
const GRIP: f32 = 12.0;

pub struct Resizable {
    pub(crate) id: ElementId,
    pub(crate) axis: ResizeAxis,
    pub(crate) build: Rc<dyn Fn(Option<f32>, Option<f32>) -> AnyElement>,
}

impl Resizable {
    pub fn new(
        id: ElementId,
        axis: ResizeAxis,
        build: Rc<dyn Fn(Option<f32>, Option<f32>) -> AnyElement>,
    ) -> Self {
        Resizable { id, axis, build }
    }
}

impl Element for Resizable {
    type RequestLayoutState = AnyElement;
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
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        grip: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        reblessive::Stack::new()
            .enter(|stack| {
                self.paint_async(id, _inspector_id, bounds, child, grip, window, cx, stack)
            })
            .finish();
    }

    fn paint_async<'a>(
        &'a mut self,
        id: Option<&'a GlobalElementId>,
        _inspector_id: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &'a mut AnyElement,
        grip: &'a mut Hitbox,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            child.paint_async(window, cx, stack).await;
            let Some(global_id) = id else { return };
            let axis = self.axis;
            let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));

            // Курсор над уголком показывает, что его можно тянуть.
            if grip.is_hovered(window) {
                window.set_cursor_style(
                    match axis {
                        ResizeAxis::Horizontal => gpui::CursorStyle::ResizeLeftRight,
                        ResizeAxis::Vertical => gpui::CursorStyle::ResizeUpDown,
                        ResizeAxis::Both => gpui::CursorStyle::ResizeUpLeftDownRight,
                    },
                    grip,
                );
            }

            let hovered = grip.is_hovered(window);
            window.with_element_state::<State, _>(global_id, |state, window| {
                let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));

                let down = st.clone();
                window.on_mouse_event(move |e: &MouseDownEvent, phase, _window, _cx| {
                    if !phase.bubble() || e.button != MouseButton::Left || !hovered {
                        return;
                    }
                    let mut s = down.get();
                    s.dragging = true;
                    s.from = (s.width.unwrap_or(size.0), s.height.unwrap_or(size.1));
                    s.at = (f32::from(e.position.x), f32::from(e.position.y));
                    down.set(s);
                });

                let mv = st.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
                    if !phase.bubble() {
                        return;
                    }
                    let mut s = mv.get();
                    if !s.dragging {
                        return;
                    }
                    let dx = f32::from(e.position.x) - s.at.0;
                    let dy = f32::from(e.position.y) - s.at.1;
                    if matches!(axis, ResizeAxis::Both | ResizeAxis::Horizontal) {
                        s.width = Some((s.from.0 + dx).max(GRIP));
                    }
                    if matches!(axis, ResizeAxis::Both | ResizeAxis::Vertical) {
                        s.height = Some((s.from.1 + dy).max(GRIP));
                    }
                    mv.set(s);
                    window.refresh();
                });

                super::drag::release_on_mouse_up(st.clone(), |s| &mut s.dragging, window);

                ((), st.get())
            });
        }
    }
}

impl IntoElement for Resizable {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
