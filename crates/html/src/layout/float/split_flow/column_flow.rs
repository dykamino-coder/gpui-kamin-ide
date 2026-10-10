//! ColumnFlow как элемент gpui: структура и раскладка колонок.

use super::columns::{intrinsic_column_width, measure_columns};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, Font, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Style, Window, px, size,
};
use std::rc::Rc;

/// Многоколоночный поток: текст режется на колонки по строкам.
///
/// Сетка из детей давала то же расположение только когда детей много: один
/// длинный абзац оставался в первой колонке целиком. Настоящий поток режет
/// сам текст — сколько строк на колонку, столько и уходит, остальное в
/// следующую. Где кончается строка, знает перенос, а он зависит от ширины
/// колонки: значит, снова замер.
pub struct ColumnFlow {
    pub(super) build: Rc<dyn Fn(&[usize], usize, Pixels) -> AnyElement>,
    pub(super) text: SharedString,
    pub(super) count: Option<usize>,
    pub(super) col_w: Option<f32>,
    pub(super) gap: f32,
    pub(super) font: Font,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    /// `column-fill: auto` с заданной высотой: колонки заполняются подряд до
    /// этой высоты, а не делятся поровну (css-multicol-1 §3.3).
    pub(super) fill_height: Option<f32>,
    /// Текст — единственного ребёнка-монолита (`render::column_flow_in`): в
    /// узкой колонке его строки не режутся (`measure_columns`).
    pub(super) whole: bool,
    /// `orphans`/`widows` блока со строками (css-break-3 §4.4).
    pub(super) line_breaks: (usize, usize),
    pub(super) cuts: Rc<std::cell::RefCell<(Vec<usize>, Pixels, usize)>>,
    pub(super) child: Option<AnyElement>,
}

impl ColumnFlow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        build: Rc<dyn Fn(&[usize], usize, Pixels) -> AnyElement>,
        text: SharedString,
        count: Option<usize>,
        col_w: Option<f32>,
        gap: f32,
        font: Font,
        font_size: f32,
        line_height: f32,
        fill_height: Option<f32>,
        whole: bool,
    ) -> Self {
        ColumnFlow {
            build,
            text,
            count,
            col_w,
            gap,
            font,
            font_size,
            line_height,
            fill_height,
            whole,
            line_breaks: (1, 1),
            cuts: Rc::new(std::cell::RefCell::new((Vec::new(), px(0.), 1))),
            child: None,
        }
    }

    /// `orphans`/`widows` строк потока (css-break-3 §4.4): сколько строк блока
    /// должно остаться в колонке до разрыва и после него.
    pub fn line_breaks(mut self, orphans: usize, widows: usize) -> Self {
        self.line_breaks = (orphans.max(1), widows.max(1));
        self
    }
}

impl Element for ColumnFlow {
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
        let count = self.count;
        let col_w = self.col_w;
        let gap = self.gap;
        let font = self.font.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let cuts = self.cuts.clone();
        let fill_height = self.fill_height;
        let whole = self.whole;
        let line_breaks = self.line_breaks;
        // css-multicol-1 §Overflow: заданная блочная высота ограничивает высоту
        // КОЛОНКИ, а не всей стопки — с ней рождаются переполняющие колонки.
        let layout_id = window.request_measured_layout_with_baselines(
            Style::default(),
            move |known, available, window, _cx| {
                // Под `min-content`/`max-content` доступного места НЕТ, и
                // «ширина окна» здесь была выдумкой: `width: min-content` у
                // многоколоночника не значил ничего, и коробка растягивалась
                // на весь кадр (`multicol-width-004/005` — все четыре
                // `<article>` во всю ширину). Считаем внутренний размер по
                // css-sizing-4 §multicol-intrinsic, как Blink
                // `column_layout_algorithm.cc:433`.
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(w) => w,
                    other => intrinsic_column_width(
                        &text,
                        count,
                        col_w,
                        gap,
                        &font,
                        font_size,
                        matches!(other, AvailableSpace::MinContent),
                        window,
                    ),
                });
                let (at, used, height, first_col_lines) = measure_columns(
                    &text,
                    count,
                    col_w,
                    gap,
                    &font,
                    font_size,
                    line_height,
                    fill_height,
                    whole,
                    line_breaks,
                    width,
                    window,
                );
                *cuts.borrow_mut() = (at, width, used);
                // css-align-3 §baseline-export, multi-column containers: первый
                // набор базовых линий — у колонки с самой ВЕРХНЕЙ линией (все
                // колонки начинаются сверху: первая строка), последний — у
                // колонки с самой НИЖНЕЙ, то есть последняя строка первой,
                // самой длинной колонки. Прежде текстовый поток базовой линии
                // не отдавал вовсе, и строчная коробка-многоколоночник
                // выравнивалась нижним краем (`baseline-008`).
                let id = window.text_system().resolve_font(&font);
                let ascent = window.text_system().ascent(id, px(font_size));
                let descent = window.text_system().descent(id, px(font_size));
                let first = (px(line_height) - (ascent + descent.abs())) / 2.0 + ascent;
                let last = first + px(line_height * first_col_lines.saturating_sub(1) as f32);
                (size(width, height), Some(first), Some(last))
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
        let stale = self.cuts.borrow().1 != bounds.size.width;
        if stale && bounds.size.width > px(0.) {
            let (at, used, _, _) = measure_columns(
                &self.text,
                self.count,
                self.col_w,
                self.gap,
                &self.font,
                self.font_size,
                self.line_height,
                self.fill_height,
                self.whole,
                self.line_breaks,
                bounds.size.width,
                window,
            );
            *self.cuts.borrow_mut() = (at, bounds.size.width, used);
        }
        let (cuts, used) = {
            let b = self.cuts.borrow();
            (b.0.clone(), b.2)
        };
        let mut child = (self.build)(&cuts, used, bounds.size.width);
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

impl IntoElement for ColumnFlow {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
