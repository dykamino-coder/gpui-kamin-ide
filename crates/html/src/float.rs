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
            .wrap_line(&[LineFragment::text(body)], width)
            .count()
            + 1;
        return (0, px(float_size.1 + below as f32 * line_height));
    }
    let beside_lines = (float_size.1 / line_height).ceil().max(1.0) as usize;
    let at = wrapper
        .wrap_line(&[LineFragment::text(body)], px(narrow))
        .nth(beside_lines - 1)
        .map(|b| lead + b.ix)
        // Текст кончился раньше, чем плавающий блок: резать нечего.
        .unwrap_or(text.len());
    let below_lines = if at >= text.len() {
        0
    } else {
        wrapper
            .wrap_line(
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
    build: Rc<dyn Fn(&[usize], usize, Pixels) -> AnyElement>,
    text: SharedString,
    count: Option<usize>,
    col_w: Option<f32>,
    gap: f32,
    font: Font,
    font_size: f32,
    line_height: f32,
    /// `column-fill: auto` с заданной высотой: колонки заполняются подряд до
    /// этой высоты, а не делятся поровну (css-multicol-1 §3.3).
    fill_height: Option<f32>,
    cuts: Rc<std::cell::RefCell<(Vec<usize>, Pixels, usize)>>,
    child: Option<AnyElement>,
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
            cuts: Rc::new(std::cell::RefCell::new((Vec::new(), px(0.), 1))),
            child: None,
        }
    }
}

/// Места разрезов на колонки и высота потока при заданной ширине.
#[allow(clippy::too_many_arguments)]
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v164, `scout-multicol-2026-09.md` MC-BALANCE-CAP,
/// 18 хунков в `flow.rs`/`float.rs`/`render.rs`): заданная блочная высота
/// многоколоночника как потолок высоты КОЛОНКИ (`cap_height` здесь,
/// `balance_line(kids, count, cap)` в `ColumnStack`), чтобы рождались
/// переполняющие колонки по css-multicol-1 §Overflow. Обещание 4…11. Полный
/// свод против v36: +3 (`multicol-fill-balance-041`,
/// `multicol-gap-decorations-005`, `out-of-flow-in-multicolumn-003/007/082`)
/// / −9 (`column-height-025/026/027`, `multicol-fill-balance-003/030`,
/// `-nested-000`, `multicol-nested-021/031`,
/// `fixed-in-nested-multicol-with-viewport-container` → «красное видно»).
/// Потолок ломает вложенные многоколоночники: внешняя высота режет
/// ВНУТРЕННИЙ, у которого своя балансировка. Возвращать только вместе с
/// MC-NESTED (фрагментация вложенного многоколоночника внешним).
fn measure_columns(
    text: &str,
    count: Option<usize>,
    col_w: Option<f32>,
    gap: f32,
    font: &Font,
    font_size: f32,
    line_height: f32,
    fill_height: Option<f32>,
    width: Pixels,
    window: &mut Window,
) -> (Vec<usize>, usize, Pixels) {
    // Used column-count по фактической ширине (css-multicol §3.4,
    // ResolveUsedColumnCount): `columns: auto <w>` до замера не решается.
    let avail = f32::from(width);
    let from_width = col_w
        .filter(|w| *w > 0.0)
        .map(|w| (((avail + gap) / (w + gap)).floor().max(1.0)) as usize);
    let count = match (count, from_width) {
        (Some(c), Some(fw)) => c.min(fw),
        (Some(c), None) => c,
        (None, Some(fw)) => fw,
        (None, None) => 1,
    };
    let inner = (avail - gap * (count.saturating_sub(1)) as f32) / count as f32;
    if inner <= font_size {
        return (Vec::new(), count, px(line_height));
    }
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    // Жёсткие разрывы приходят как символ новой строки: каждый сегмент
    // переносится отдельно, начало сегмента — принудительная граница.
    let mut boundaries: Vec<usize> = Vec::new();
    let mut off = 0usize;
    for (i, seg) in text.split('\n').enumerate() {
        if i > 0 {
            boundaries.push(off);
        }
        boundaries.extend(
            wrapper
                .wrap_line(&[LineFragment::text(seg)], px(inner))
                .map(|b| b.ix + off),
        );
        off += seg.len() + 1;
    }
    let lines = boundaries.len() + 1;
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09): `column-fill: auto` с заданной высотой —
    // колонки заполняются ПОДРЯД до высоты фрагментатора (css-multicol-1
    // §3.3), то есть `per_col = floor(высота / высота строки)`, а не поровну.
    // Высота протягивалась в `ColumnFlow` из `column_flow` (`render.rs`).
    // Срез css-break+css-multicol (1498 пар, 341 зелёная): 342, приобретено
    // 21, потеряно 20. Патч — `target/column-fill.patch`.
    //
    // Важнее самих чисел совпадение: ровно ТЕ ЖЕ двадцать пар
    // (`overflow-clip-004`, `table-cell-expansion-006`,
    // `flex-container-fragmentation-008/009`, `monolithic-with-overflow`,
    // `out-of-flow-in-multicolumn-120/127`, `overflowing-block-003`,
    // `box-shadow-001`, `become-unfragmented-001`) рушатся и от разреза
    // ребёнка по краю колонки (запись у `ColumnStack` в `flow.rs`) — при том
    // что правки совершенно разные. Значит, они зелены не потому, что мы
    // фрагментируем верно, а потому, что не фрагментируем вовсе, и любой
    // ЧАСТИЧНЫЙ шаг их ломает. Отсюда порядок работ: фрагментацию делать
    // одним куском (высота фрагментатора + разрыв между блочными детьми
    // РЕКУРСИВНО + монолиты), а не по частям; поштучные заходы измеримо
    // упираются в +1.
    // `column-fill: auto` (css-multicol-1 §3.3): колонки заполняются ПОДРЯД
    // до высоты фрагментатора, а не делятся поровну. Пока высота не
    // учитывалась вовсе, и заданная высота коробки не влияла на разрезы:
    // строки распределялись ровно по числу колонок.
    let per_col = match fill_height {
        Some(h) if h >= line_height => ((h / line_height).floor() as usize).max(1),
        _ => lines.div_ceil(count).max(1),
    };
    let cuts: Vec<usize> = (1..count)
        .filter_map(|i| boundaries.get(i * per_col - 1).copied())
        .collect();
    (cuts, count, px(per_col as f32 * line_height))
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
        // css-multicol-1 §Overflow: заданная блочная высота ограничивает высоту
        // КОЛОНКИ, а не всей стопки — с ней рождаются переполняющие колонки.
        let layout_id = window.request_measured_layout(
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
                let (at, used, height) = measure_columns(
                    &text,
                    count,
                    col_w,
                    gap,
                    &font,
                    font_size,
                    line_height,
                    fill_height,
                    width,
                    window,
                );
                *cuts.borrow_mut() = (at, width, used);
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
            let (at, used, _) = measure_columns(
                &self.text,
                self.count,
                self.col_w,
                self.gap,
                &self.font,
                self.font_size,
                self.line_height,
                self.fill_height,
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

/// Внутренний размер многоколоночного контейнера с ТЕКСТОВЫМ потоком.
///
/// css-sizing-4 `intrinsic-sizing-notes.bs` §multicol-intrinsic — единственное
/// письменное определение (css-multicol-1 §3.4 прямо отказывается его давать).
/// Порядок действий — как в Blink `ColumnLayoutAlgorithm::ComputeMinMaxSizes`.
/// Вклад содержимого у текста берут те же метрики, что и перенос
/// (`LineWrapper::min_content_width` / `max_content_width`), иначе замер и
/// перенос разойдутся между собой.
#[allow(clippy::too_many_arguments)]
fn intrinsic_column_width(
    text: &str,
    count: Option<usize>,
    col_w: Option<f32>,
    gap: f32,
    font: &Font,
    font_size: f32,
    min: bool,
    window: &mut Window,
) -> Pixels {
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    let (mut kid_min, mut kid_max) = (0.0f32, 0.0f32);
    // Жёсткие разрывы приходят переводом строки: каждый сегмент — свой абзац,
    // и вклад даёт самый широкий из них.
    for seg in text.split('\n') {
        kid_min = kid_min.max(f32::from(wrapper.min_content_width(seg)));
        kid_max = kid_max.max(f32::from(wrapper.max_content_width(seg)));
    }
    let n = count.unwrap_or(1).max(1) as f32;
    let gap_extra = gap * (n - 1.0);
    let (mut mn, mut mx) = (kid_min, kid_max);
    match col_w.filter(|w| *w > 0.0) {
        Some(w) => {
            mn = mn.min(w);
            mx = mx.max(w).max(mn);
        }
        None => mn = mn * n + gap_extra,
    }
    mx = mx * n + gap_extra;
    px(if min { mn } else { mx })
}
