//! Обтекание плавающего блока текстом.
//!
//! Ряд из двух колонок передаёт главное — «картинка слева, текст справа», — но
//! в браузере текст, кончив обтекать, возвращается под плавающий блок на всю
//! ширину. Ряд этого не умеет: колонка так и остаётся узкой до конца абзаца.
//!
//! Здесь текст режется надвое по месту, где кончается плавающий блок. Место
//! ищется тем же переносчиком, что и обычная строка: сколько строк узкой
//! ширины помещается в высоту блока, столько и уходит вбок, остальное встаёт
//! под ним на всю ширину.
//!
//! Ширина контейнера известна только замеру, а собирать дерево на замере
//! нельзя — раскладка в этот момент занята. Поэтому замер считает ТОЛЬКО место
//! разреза и высоту, а дерево собирается в подготовке к отрисовке, где ширина
//! уже известна из коробки элемента.

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, Font, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, LineFragment, Pixels, SharedString, Style, Window,
    px, size,
};
use std::cell::Cell;
use std::rc::Rc;

/// Строит поддерево по месту разреза текста (в байтах).
pub type Split = Rc<dyn Fn(usize, Pixels) -> AnyElement>;

/// Где текст расстаётся с плавающим блоком.
#[derive(Clone, Copy, Default)]
struct Cut {
    /// Смещение разреза в байтах.
    at: usize,
    /// Ширина коробки на замере — по ней собирается дерево.
    width: Pixels,
}

pub struct FloatFlow {
    build: Split,
    text: SharedString,
    /// Ширина и высота плавающего блока в точках.
    float_size: (f32, f32),
    font: Font,
    font_size: f32,
    line_height: f32,
    cut: Rc<Cell<Cut>>,
    child: Option<AnyElement>,
}

impl FloatFlow {
    pub fn new(
        build: Split,
        text: SharedString,
        float_size: (f32, f32),
        font: Font,
        font_size: f32,
        line_height: f32,
    ) -> Self {
        FloatFlow {
            build,
            text,
            float_size,
            font,
            font_size,
            line_height,
            cut: Rc::new(Cell::new(Cut::default())),
            child: None,
        }
    }
}

/// Место разреза и высота ряда при заданной ширине.
#[allow(clippy::too_many_arguments)]
fn measure(
    text: &str,
    float_size: (f32, f32),
    font: &Font,
    font_size: f32,
    line_height: f32,
    width: Pixels,
    window: &mut Window,
) -> (usize, Pixels) {
    let narrow = f32::from(width) - float_size.0;
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    // Сбоку не помещается даже несколько букв — весь текст идёт под блоком.
    if narrow <= font_size * 4.0 {
        let below = wrapper
            .wrap_line(&[LineFragment::text(text)], width)
            .count()
            + 1;
        return (0, px(float_size.1 + below as f32 * line_height));
    }
    let beside_lines = (float_size.1 / line_height).ceil().max(1.0) as usize;
    let at = wrapper
        .wrap_line(&[LineFragment::text(text)], px(narrow))
        .nth(beside_lines - 1)
        .map(|b| b.ix)
        // Текст кончился раньше, чем плавающий блок: резать нечего.
        .unwrap_or(text.len());
    let below_lines = if at >= text.len() {
        0
    } else {
        wrapper
            .wrap_line(&[LineFragment::text(&text[at..])], width)
            .count()
            + 1
    };
    let top = float_size.1.max(beside_lines as f32 * line_height);
    (at, px(top + below_lines as f32 * line_height))
}

impl Element for FloatFlow {
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
        let float_size = self.float_size;
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
                let (at, height) = measure(
                    &text,
                    float_size,
                    &font,
                    font_size,
                    line_height,
                    width,
                    window,
                );
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
        // Ширина коробки точнее замерной: там она была лишь доступным местом.
        let at = if bounds.size.width != self.cut.get().width && bounds.size.width > px(0.) {
            measure(
                &self.text,
                self.float_size,
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

impl IntoElement for FloatFlow {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Абзац, у которого первая строка набрана своим стилем (`::first-line`).
///
/// Где кончается первая строка, известно только после переноса, а перенос
/// зависит от ширины коробки — то есть от замера. Поэтому абзац собирается
/// дважды: замер считает длину первой строки, а подготовка к отрисовке
/// собирает по ней прогоны.
pub struct FirstLine {
    build: Split,
    text: SharedString,
    font: Font,
    font_size: f32,
    line_height: f32,
    cut: Rc<Cell<Cut>>,
    child: Option<AnyElement>,
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
fn measure_first_line(
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
        .wrap_line(&[LineFragment::text(text)], width)
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

/// Многоколоночный поток: текст режется на колонки по строкам.
///
/// Сетка из детей давала то же расположение только когда детей много: один
/// длинный абзац оставался в первой колонке целиком. Настоящий поток режет
/// сам текст — сколько строк на колонку, столько и уходит, остальное в
/// следующую. Где кончается строка, знает перенос, а он зависит от ширины
/// колонки: значит, снова замер.
pub struct ColumnFlow {
    build: Rc<dyn Fn(&[usize], Pixels) -> AnyElement>,
    text: SharedString,
    count: usize,
    gap: f32,
    font: Font,
    font_size: f32,
    line_height: f32,
    cuts: Rc<std::cell::RefCell<(Vec<usize>, Pixels)>>,
    child: Option<AnyElement>,
}

impl ColumnFlow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        build: Rc<dyn Fn(&[usize], Pixels) -> AnyElement>,
        text: SharedString,
        count: usize,
        gap: f32,
        font: Font,
        font_size: f32,
        line_height: f32,
    ) -> Self {
        ColumnFlow {
            build,
            text,
            count,
            gap,
            font,
            font_size,
            line_height,
            cuts: Rc::new(std::cell::RefCell::new((Vec::new(), px(0.)))),
            child: None,
        }
    }
}

/// Места разрезов на колонки и высота потока при заданной ширине.
#[allow(clippy::too_many_arguments)]
fn measure_columns(
    text: &str,
    count: usize,
    gap: f32,
    font: &Font,
    font_size: f32,
    line_height: f32,
    width: Pixels,
    window: &mut Window,
) -> (Vec<usize>, Pixels) {
    let inner = (f32::from(width) - gap * (count.saturating_sub(1)) as f32) / count as f32;
    if inner <= font_size {
        return (Vec::new(), px(line_height));
    }
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    let boundaries: Vec<usize> = wrapper
        .wrap_line(&[LineFragment::text(text)], px(inner))
        .map(|b| b.ix)
        .collect();
    let lines = boundaries.len() + 1;
    let per_col = lines.div_ceil(count).max(1);
    let cuts: Vec<usize> = (1..count)
        .filter_map(|i| boundaries.get(i * per_col - 1).copied())
        .collect();
    (cuts, px(per_col as f32 * line_height))
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
        let gap = self.gap;
        let font = self.font.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let cuts = self.cuts.clone();
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known, available, window, _cx| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(w) => w,
                    _ => window.viewport_size().width,
                });
                let (at, height) = measure_columns(
                    &text,
                    count,
                    gap,
                    &font,
                    font_size,
                    line_height,
                    width,
                    window,
                );
                *cuts.borrow_mut() = (at, width);
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
        let stale = self.cuts.borrow().1 != bounds.size.width;
        if stale && bounds.size.width > px(0.) {
            let (at, _) = measure_columns(
                &self.text,
                self.count,
                self.gap,
                &self.font,
                self.font_size,
                self.line_height,
                bounds.size.width,
                window,
            );
            *self.cuts.borrow_mut() = (at, bounds.size.width);
        }
        let cuts = self.cuts.borrow().0.clone();
        let mut child = (self.build)(&cuts, bounds.size.width);
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
