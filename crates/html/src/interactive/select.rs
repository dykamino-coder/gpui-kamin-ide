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
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, SharedString, StyledText, TextLayout, TextRun, Window,
};

/// Память между кадрами: границы выделения в байтах строки.
#[derive(Default, Clone, Copy)]
struct State {
    anchor: usize,
    head: usize,
    dragging: bool,
}

impl State {
    fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
}

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
        if from >= to {
            return self.runs.clone();
        }
        let mut out = Vec::with_capacity(self.runs.len() + 2);
        let mut at = 0usize;
        for run in &self.runs {
            let end = at + run.len;
            let mut cut = |start: usize, stop: usize, selected: bool| {
                if stop <= start {
                    return;
                }
                let mut piece = run.clone();
                piece.len = stop - start;
                piece.background_color = selected.then_some(self.highlight);
                out.push(piece);
            };
            cut(at, end.min(from), false);
            cut(at.max(from), end.min(to), true);
            cut(at.max(to), end, false);
            at = end;
        }
        out
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
        let Some(global_id) = id else { return };
        let Some(layout) = self.layout.clone() else {
            return;
        };
        // Над текстом курсор — как в браузере, каретка.
        if hitbox.is_hovered(window) {
            window.set_cursor_style(gpui::CursorStyle::IBeam, hitbox);
        }
        let inside = hitbox.is_hovered(window);

        window.with_element_state::<State, _>(global_id, |state, window| {
            let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));
            // Индекс символа под точкой; за пределами строки — ближайший край.
            let index_at = {
                let layout = layout.clone();
                move |p| match layout.index_for_position(p) {
                    Ok(i) => Some(i),
                    Err(i) => Some(i),
                }
            };

            let down = st.clone();
            let at_down = index_at.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _cx| {
                if !phase.bubble() || e.button != MouseButton::Left || !inside {
                    return;
                }
                let Some(i) = at_down(e.position) else { return };
                down.set(State {
                    anchor: i,
                    head: i,
                    dragging: true,
                });
                window.refresh();
            });

            let mv = st.clone();
            let at_move = index_at.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = mv.get();
                if !s.dragging {
                    return;
                }
                if let Some(i) = at_move(e.position)
                    && s.head != i
                {
                    s.head = i;
                    mv.set(s);
                    window.refresh();
                }
            });

            let up = st.clone();
            window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = up.get();
                if s.dragging {
                    s.dragging = false;
                    up.set(s);
                }
            });

            ((), st.get())
        });
    }
}

impl IntoElement for Selectable {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
