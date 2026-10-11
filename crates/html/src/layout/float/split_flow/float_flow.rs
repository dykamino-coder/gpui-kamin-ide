//! FloatFlow: поток с флоатами, раскладка при известной ширине (measure).

use super::{Cut, Split};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, Font, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, LineFragment, Pixels, SharedString, Style, Window,
    px, size,
};
use std::cell::Cell;
use std::rc::Rc;

pub struct FloatFlow {
    pub(super) build: Split,
    pub(super) text: SharedString,
    /// Ширина и высота плавающего блока в точках.
    pub(super) float_size: (f32, f32),
    pub(super) font: Font,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    pub(super) cut: Rc<Cell<Cut>>,
    pub(super) child: Option<AnyElement>,
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
pub(super) fn measure(
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
    // Сбоку не помещается НИЧЕГО — весь текст идёт под блоком. Мерой служит
    // сам остаток, а не «четыре кегля»: §9.5 сужает строку рядом с флоатом,
    // пока в неё влезает хоть одно слово. Прежний порог уводил вниз текст,
    // который помещался (проба `probe/flt-probe.html`: флоат 100 в контейнере
    // 200 и слова «xx» по 100 точек — текст уезжал под флоат).
    //
    // ЗАМЕРЕНО дважды: срез из 1828 пар флоатов и форм против свода v20 —
    // 1118 -> 1118, ноль сдвигов в обе стороны. Прошлый замер этой же правки
    // показывал −35, но те потери принадлежали чужому гейту `align-self`
    // (снят в 82dfc0f), а не порогу.
    // Ведущий пробельный прогон — ВНЕ переносчика. `LineWrapper::wrap_line`
    // (vendor/gpui `line_wrapper.rs:201-217`) редакторский: знаки до первого
    // непробельного он запоминает ОТСТУПОМ и прибавляет `отступ × ширина
    // пробела` к КАЖДОЙ перенесённой строке. Текст колонки приходит сырым
    // (`gather_text`): «\n  XXXXX …» давал отступ 3, в Ahem 20px строка после
    // первой теряла 60 точек из 100, слово рвалось по буквам, и разрез уходил
    // на 25-й байт вместо конца текста (`shape-outside-path-000-ref`: сбоку
    // 19 «X» из 50, остальное под флоатом). В CSS такого отступа нет: пробелы
    // в начале строки удаляются (CSS 2.1 §16.6.1, css-text-3 §4.1.2), и
    // `lines.rs` рисует колонку без него. Смещения переносчика возвращаются в
    // сырой текст прибавкой длины прогона — `split_nodes` режет именно его.
    let ws = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r');
    let lead = text.len() - text.trim_start_matches(ws).len();
    let body = &text[lead..];
    if narrow <= 0.0 {
        let below = wrapper
            .wrap_line_css(&[LineFragment::text(body)], width)
            .count()
            + 1;
        return (0, px(float_size.1 + below as f32 * line_height));
    }
    let beside_lines = (float_size.1 / line_height).ceil().max(1.0) as usize;
    let at = wrapper
        .wrap_line_css(&[LineFragment::text(body)], px(narrow))
        .nth(beside_lines - 1)
        .map(|b| lead + b.ix)
        // Текст кончился раньше, чем плавающий блок: резать нечего.
        .unwrap_or(text.len());
    let below_lines = if at >= text.len() {
        0
    } else {
        wrapper
            .wrap_line_css(
                &[LineFragment::text(text[at..].trim_start_matches(ws))],
                width,
            )
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
