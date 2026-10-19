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
        let selection = match id {
            Some(global_id) => window.with_element_state::<State, _>(global_id, |state, _| {
                let st = state.unwrap_or_default();
                (st.range(), st)
            }),
            None => ((0, 0), State::default()).0,
        };
        let styled = StyledText::new(self.text.clone())
            .with_ligature_breaking(false)
            .with_runs(self.runs_with_selection(selection.0, selection.1));
        // Раскладка нужна на этапе отрисовки, чтобы попасть точкой в символ.
        self.layout = Some(styled.layout().clone());
        let mut el = styled.into_any_element();
        let layout_id = el.request_layout(window, cx);
        (layout_id, el)
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
        crate::interactive::hitbox::prepaint_hitbox(child, bounds, window, cx)
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

impl IntoElement for Selectable {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
