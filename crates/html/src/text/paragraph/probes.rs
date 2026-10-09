//! Пробы и записи раскладки: места атомов, экстенты руби, оформление, базовые линии, LayoutTap.

use gpui::{AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Window, px, size};

/// Как атом встаёт в строке по вертикали (`vertical-align`, CSS 2.1 §10.8.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AtomAlign {
    /// По базовой линии родителя, поднятой на `v` точек (`baseline` — ноль;
    /// длина, процент, `sub`/`super` — свой подъём).
    Shift(f32),
    /// Середина коробки — на высоте базовой линии плюс половина x-высоты.
    Middle,
    /// Верх коробки — по верху текстовой области родителя.
    TextTop,
    /// Низ коробки — по низу текстовой области родителя.
    TextBottom,
    /// Верх коробки — по верху строчной коробки.
    Top,
    /// Низ коробки — по низу строчной коробки.
    Bottom,
}

/// Атом в строке: элемент-обёртка со щупом базовой линии.
pub(crate) struct AtomSlot {
    pub(crate) at: usize,
    pub(crate) el: AnyElement,
    pub(crate) align: AtomAlign,
    pub(crate) probe: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
    /// Узел самой обёртки (см. `LayoutTap`).
    pub(crate) root: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
    /// Узлы уровней аннотаций руби (`ruby_extent`): `true` — под базой.
    pub(crate) extents: RubyExtents,
    /// Атом за последней строкой (оборван `line-clamp`): не ставится и не
    /// рисуется.
    pub(crate) hidden: bool,
}

/// Узлы стопок аннотаций одного руби: (под базой?, полулидинг базы, узел).
/// Полулидинг вычитается: стопка стоит на краю коробки строки базы, а
/// аннотация в браузере — на краю её СОДЕРЖИМОГО.
#[derive(Default)]
pub struct RubyExtents {
    pub levels: Vec<(bool, f32, std::rc::Rc<std::cell::Cell<Option<LayoutId>>>)>,
    /// What the line needs to let the annotation overhang its neighbours
    /// (`ruby_overhang_probe`, css-ruby-1 §4.4); `None` for non-ruby atoms.
    pub overhang: Option<RubyOverhangInfo>,
}

impl RubyExtents {
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

/// Inputs of the ruby overhang computation (Blink `ruby_utils.cc`
/// `GetOverhang`): the overhang mode, half the annotation font size (the
/// `auto` limit), whether `ruby-align: start` (overhang only at the end), the
/// base content width reported by the base paragraph during layout, and the
/// base's font size (no end overhang over larger following text).
#[derive(Clone)]
pub struct RubyOverhangInfo {
    pub mode: crate::style::computed::RubyOverhang,
    pub half_annotation_font: f32,
    pub align_start: bool,
    pub base_font: f32,
    pub base_width: std::rc::Rc<std::cell::Cell<Option<f32>>>,
}

thread_local! {
    /// Sink of the base paragraph that is being built for a ruby column
    /// (`ruby_base_width_sink`): the paragraph records its max-content width
    /// there when it is measured.
    pub(crate) static RUBY_BASE_SINK: std::cell::RefCell<Option<std::rc::Rc<std::cell::Cell<Option<f32>>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Build the ruby BASE unit while a width sink is active: the paragraph
/// created inside takes the sink (`Paragraph::new`) and reports its content
/// width into it. Also registers the overhang info for the atom being
/// collected (`collect_ruby_extents`).
pub fn ruby_base_with_overhang<T>(
    mode: crate::style::computed::RubyOverhang,
    half_annotation_font: f32,
    align_start: bool,
    base_font: f32,
    build: impl FnOnce() -> T,
) -> T {
    let sink = std::rc::Rc::new(std::cell::Cell::new(None));
    let saved = RUBY_BASE_SINK.with(|s| s.replace(Some(sink.clone())));
    let out = build();
    RUBY_BASE_SINK.with(|s| s.replace(saved));
    RUBY_EXTENTS.with(|r| {
        if let Some(v) = r.borrow_mut().as_mut()
            && v.overhang.is_none()
        {
            v.overhang = Some(RubyOverhangInfo {
                mode,
                half_annotation_font,
                align_start,
                base_font,
                base_width: sink,
            });
        }
    });
    out
}

/// Take the active base-width sink (for the first paragraph built under it).
pub(crate) fn take_ruby_base_sink() -> Option<std::rc::Rc<std::cell::Cell<Option<f32>>>> {
    RUBY_BASE_SINK.with(|s| s.borrow_mut().take())
}

thread_local! {
    /// Сбор узлов аннотаций для атома, который сейчас строится
    /// (`collect_ruby_extents`). `None` — сбора нет: руби вне строки абзаца
    /// своих аннотаций никому не отдаёт.
    pub(crate) static RUBY_EXTENTS: std::cell::RefCell<Option<RubyExtents>> =
        const { std::cell::RefCell::new(None) };
}

/// Построить атом, собрав узлы аннотаций руби, которые он заведёт.
/// Вложенный сбор (атом внутри атома) своё забирает сам: прежний список
/// восстанавливается после вызова.
pub fn collect_ruby_extents<T>(build: impl FnOnce() -> T) -> (T, RubyExtents) {
    let saved = RUBY_EXTENTS.with(|r| r.replace(Some(RubyExtents::default())));
    let out = build();
    let mine = RUBY_EXTENTS.with(|r| r.replace(saved)).unwrap_or_default();
    (out, mine)
}

/// Получает ли знак `c` метку акцента (css-text-decor-3 §5.3): нет у
/// разделителей (Z*), управляющих и неназначенных (Cc, Cf, Cn) и у
/// пунктуации (P*), кроме знаков, что по NFKD сводятся к `#`, `%`, `‰`, `‱`,
/// `٪`, `؉`, `؊`, `&`, `⁊`, `@`, `§`, `¶`, `⁋`, `⁓`, `〽` (здесь — сами они и
/// их полноширинные и малые формы).
pub(crate) fn emphasized(c: char) -> bool {
    use unicode_properties::{GeneralCategory as G, GeneralCategoryGroup, UnicodeGeneralCategory};
    if c.is_whitespace() {
        return false;
    }
    match c.general_category_group() {
        GeneralCategoryGroup::Separator => false,
        GeneralCategoryGroup::Other => !matches!(
            c.general_category(),
            G::Control | G::Format | G::Unassigned
        ),
        GeneralCategoryGroup::Punctuation => matches!(
            c,
            '#' | '%' | '\u{2030}' | '\u{2031}' | '\u{066A}' | '\u{0609}' | '\u{060A}' | '&'
                | '\u{204A}' | '@' | '\u{00A7}' | '\u{00B6}' | '\u{204B}' | '\u{2053}'
                | '\u{303D}' | '\u{FF03}' | '\u{FF05}' | '\u{FF06}' | '\u{FF20}' | '\u{FE5F}'
                | '\u{FE6A}' | '\u{FE60}' | '\u{FE6B}'
        ),
        _ => true,
    }
}

/// Кусок текста с украшениями (css-text-decor-3 §2.1).
#[derive(Clone, Debug)]
pub struct DecorSpan {
    /// Отрезок байт текста абзаца.
    pub range: std::ops::Range<usize>,
    /// Украшения куска от внешнего к внутреннему.
    pub items: Vec<DecorItem>,
    /// `text-decoration-skip-ink` не `none` (css-text-decor-4 §4.3).
    pub skip_ink: bool,
    /// `text-decoration-skip-spaces`: 1 `start`, 2 `end`, 4 `all`.
    pub skip_spaces: u8,
}

/// Одно украшение куска.
#[derive(Clone, Debug)]
pub struct DecorItem {
    pub decor: crate::style::computed::Decor,
    /// Шрифт украшающей коробки (метрики линий).
    pub font: gpui::Font,
    /// Украшенный прогон (Blink «decorated run»): смежные куски с тем же
    /// украшением — от них считаются `text-decoration-inset` при `slice`.
    pub group: std::ops::Range<usize>,
}

/// Кусок со знаком акцента (`text-emphasis`, css-text-decor-3 §5).
#[derive(Clone, Debug)]
pub struct EmphSpan {
    /// Отрезок байт текста абзаца.
    pub range: std::ops::Range<usize>,
    /// Знак под текстом (`text-emphasis-position: under`).
    pub under: bool,
    /// Кегль знака — половина кегля базы (§5.3: как аннотация руби).
    pub size: f32,
    /// `line-height` куска в точках: аннотация встаёт на край его строчной
    /// коробки, а не на край кегля (как `<rt>` над базой руби).
    pub line_height: f32,
    /// Сам знак (первая буква строки или знак формы).
    pub mark: String,
    /// `text-emphasis-color`; пусто — цвет текста.
    pub color: Option<gpui::Hsla>,
}

/// Стопка аннотаций руби, чью высоту строка должна знать (css-ruby-1 §3.4):
/// аннотации в высоту строки не входят, но «the UA must increase the line's
/// spacing … so that the ruby annotation fits» — строка растёт ровно на то,
/// чем аннотация выходит за её коробку (Blink `ruby_utils.cc`
/// `ComputeAnnotationOverflow`). Без сбора — сам элемент как есть.
pub fn ruby_extent(el: AnyElement, under: bool, inset: f32) -> AnyElement {
    let slot = std::rc::Rc::new(std::cell::Cell::new(None));
    let collecting = RUBY_EXTENTS.with(|r| {
        r.borrow_mut()
            .as_mut()
            .map(|v| v.levels.push((under, inset, slot.clone())))
            .is_some()
    });
    if !collecting {
        return el;
    }
    LayoutTap { child: el, slot }.into_any_element()
}

/// Замер атома в точках: высота коробки полей (по §10.8 выравнивается
/// именно она) и базовая линия от её верха. Ширина уходит продвижением
/// распорки (`letter_spans`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct AtomBox {
    pub(crate) at: usize,
    pub(crate) h: f32,
    pub(crate) base: f32,
    pub(crate) align: AtomAlign,
    /// Аннотации руби над и под коробкой (`ruby_extent`).
    pub(crate) over: f32,
    pub(crate) under: f32,
    /// Ruby annotation overhang onto the preceding content (css-ruby-1 §4.4):
    /// the atom is placed this far left of its spacer.
    pub(crate) shift: f32,
}

/// Щуп базовой линии атома: пустой лист с базовой линией на своём верху.
/// В ряду `align-items: baseline` рядом с атомом его верх встаёт ровно на
/// базовую линию атома, и раскладка отдаёт её положением щупа. Для атома без
/// базовой линии taffy берёт нижний край полей (`flexbox.rs`) — у замещаемого
/// это и есть его базовая по §10.8.1.
pub(crate) struct BaselineProbe {
    pub(crate) slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
}

impl Element for BaselineProbe {
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
        let id = window
            .request_measured_layout_with_baselines(gpui::Style::default(), |_, _, _, _| {
                (size(px(0.), px(0.)), Some(px(0.)), Some(px(0.)))
            });
        self.slot.set(Some(id));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for BaselineProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Обёртка, запоминающая узел раскладки своего ребёнка: по нему берётся
/// ТОЧНЫЙ размер атома (`Window::layout_exact`) — округлённый к точке
/// устройства прибавлял до 0.4px на атом, и ряд атомов ровно в ширину строки
/// в неё уже не влезал (`c542-letter-sp-001-ref`, `c5505-mrgn-000`).
pub(crate) struct LayoutTap {
    pub(crate) child: AnyElement,
    pub(crate) slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
}

impl Element for LayoutTap {
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
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let id = self.child.request_layout(window, cx);
        self.slot.set(Some(id));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
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
        self.child.paint(window, cx);
    }
}

impl IntoElement for LayoutTap {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
