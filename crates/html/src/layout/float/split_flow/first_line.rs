//! FirstLine: первая строка, отделённая от остального потока (::first-line).

use super::{Cut, Split};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, Font, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, LineFragment, Pixels, SharedString, Style, Window,
    px, size,
};
use std::cell::Cell;
use std::rc::Rc;

/// Абзац, у которого первая строка набрана своим стилем (`::first-line`).
///
/// Где кончается первая строка, известно только после переноса, а перенос
/// зависит от ширины коробки — то есть от замера. Поэтому абзац собирается
/// дважды: замер считает длину первой строки, а подготовка к отрисовке
/// собирает по ней прогоны.
pub struct FirstLine {
    pub(super) build: Split,
    pub(super) text: SharedString,
    pub(super) font: Font,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    pub(super) cut: Rc<Cell<Cut>>,
    pub(super) child: Option<AnyElement>,
}

impl FirstLine {
    pub fn new(
        build: Split,
        text: SharedString,
        font: Font,
        font_size: f32,
        line_height: f32,
    ) -> Self {
        FirstLine {
            build,
            text,
            font,
            font_size,
            line_height,
            cut: Rc::new(Cell::new(Cut::default())),
            child: None,
        }
    }
}

/// Длина первой строки и высота абзаца при заданной ширине.
pub(super) fn measure_first_line(
    text: &str,
    font: &Font,
    font_size: f32,
    line_height: f32,
    width: Pixels,
    window: &mut Window,
) -> (usize, Pixels) {
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    let boundaries: Vec<_> = wrapper
        .wrap_line_css(&[LineFragment::text(text)], width)
        .collect();
    let at = boundaries.first().map(|b| b.ix).unwrap_or(text.len());
    (at, px((boundaries.len() + 1) as f32 * line_height))
}

impl Element for FirstLine {
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
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let text = self.text.clone();
        let font = self.font.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let cut = self.cut.clone();
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known, available, window, _cx| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(w) => w,
                    _ => window.viewport_size().width,
                });
                let (at, height) =
                    measure_first_line(&text, &font, font_size, line_height, width, window);
                cut.set(Cut { at, width });
                size(width, height)
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let at = if bounds.size.width != self.cut.get().width && bounds.size.width > px(0.) {
            measure_first_line(
                &self.text,
                &self.font,
                self.font_size,
                self.line_height,
                bounds.size.width,
                window,
            )
            .0
        } else {
            self.cut.get().at
        };
        let mut child = (self.build)(at, bounds.size.width);
        child.layout_as_root(
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::MinContent,
            ),
            window,
            cx,
        );
        window.with_absolute_element_offset(bounds.origin, |window| child.prepaint(window, cx));
        self.child = Some(child);
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
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for FirstLine {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
