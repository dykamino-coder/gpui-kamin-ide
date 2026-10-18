//! FirstLine: первая строка, отделённая от остального потока (::first-line).

use super::{Cut, Split};
use crate::hooks::Hooks;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Font, LayoutId, LineFragment, Pixels, SharedString,
    Style, Window, px, size,
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
    following: Vec<SharedString>,
    normal: (Font, f32),
    pub(super) font: Font,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    /// Высота ПЕРВОЙ строки: её корневая строчная коробка берёт кегль и
    /// интерлиньяж `::first-line` (css-pseudo-4 §first-line-styling).
    first_height: Option<f32>,
    pub(super) cut: Rc<Cell<Cut>>,
}

impl FirstLine {
    pub fn new(
        build: Split,
        text: SharedString,
        font: Font,
        font_size: f32,
        line_height: f32,
        following: Vec<SharedString>,
        normal: (Font, f32),
    ) -> Self {
        FirstLine {
            build,
            text,
            following,
            normal,
            font,
            font_size,
            line_height,
            first_height: None,
            cut: Rc::new(Cell::new(Cut::default())),
        }
    }

    /// Высота первой строки, если `::first-line` её меняет.
    pub fn first_height(mut self, h: f32) -> Self {
        self.first_height = Some(h);
        self
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

impl Hooks for FirstLine {
    type Layout = ();
    type Prepaint = AnyElement;

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, ()) {
        let text = self.text.clone();
        let following = self.following.clone();
        let font = self.font.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let first_height = self.first_height;
        let normal = self.normal.clone();
        let cut = self.cut.clone();
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known, available, window, _cx| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(w) => w,
                    _ => window.viewport_size().width,
                });
                let (at, _) =
                    measure_first_line(&text, &font, font_size, line_height, width, window);
                let mut height = px(first_height.unwrap_or(line_height));
                // Only the first formatted line uses the pseudo's font.
                // Reshape its remainder with the ordinary font (CSS Pseudo 4
                // §first-line-inheritance), including its different wraps.
                let remainder = text[at..].trim_start();
                if !remainder.is_empty() {
                    height += measure_first_line(
                        remainder,
                        &normal.0,
                        normal.1,
                        line_height,
                        width,
                        window,
                    )
                    .1;
                }
                // The styled first line does not remove subsequent hard lines
                // from normal flow (CSS 2 section 10.8).
                for line in &following {
                    height +=
                        measure_first_line(line, &normal.0, normal.1, line_height, width, window).1;
                }
                cut.set(Cut { at, width });
                size(width, height)
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
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
        child
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _: &mut (),
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}
