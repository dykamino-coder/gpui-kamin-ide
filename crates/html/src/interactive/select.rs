//! Выделение текста мышью — `user-select`.
//!
//! Строится на двух готовых вещах GPUI: раскладка текста умеет отвечать, какой
//! символ находится под точкой (`TextLayout::index_for_position`), а прогон
//! текста умеет нести собственный фон (`TextRun::background_color`). Значит
//! выделение — это не новый примитив, а состояние: где начали тянуть и где
//! отпустили. Состояние живёт в памяти элемента, как и переход.
//!
//! `user-select: none` просто не заворачивает абзац в этот элемент: тогда нет
//! ни области попадания, ни обработчиков — ровно та цена, которую свойство и
//! обещает.

#[path = "select/select_traversal.rs"]
mod select_traversal;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, Hsla, InspectorElementId,
    IntoElement, LayoutId, Pixels, SharedString, StyledText, TextLayout, TextRun, Window,
};

use super::selection::{Selection as State, selected_runs, track_selection};

pub struct Selectable {
    id: ElementId,
    text: SharedString,
    runs: Vec<TextRun>,
    highlight: Hsla,
    layout: Option<TextLayout>,
}

impl Selectable {
    pub fn new(id: ElementId, text: SharedString, runs: Vec<TextRun>, highlight: Hsla) -> Self {
        Selectable {
            id,
            text,
            runs,
            highlight,
            layout: None,
        }
    }

    /// Прогоны с подложкой на выделенном куске.
    ///
    /// Прогон нельзя раскрасить наполовину, поэтому те, что попадают на
    /// границу выделения, разрезаются надвое.
    fn runs_with_selection(&self, from: usize, to: usize) -> Vec<TextRun> {
        selected_runs(&self.runs, self.highlight, from, to)
    }
}

impl Element for Selectable {
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
            let Some(global_id) = id else { return };
            let Some(layout) = self.layout.clone() else {
                return;
            };
            // Над текстом курсор — как в браузере, каретка.
            if hitbox.is_hovered(window) {
                window.set_cursor_style(gpui::CursorStyle::IBeam, hitbox);
            }
            let inside = hitbox.is_hovered(window);

            track_selection(
                global_id,
                inside,
                move |p| {
                    Some(match layout.index_for_position(p) {
                        Ok(i) | Err(i) => i,
                    })
                },
                window,
            );
        }
    }
}

impl IntoElement for Selectable {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
