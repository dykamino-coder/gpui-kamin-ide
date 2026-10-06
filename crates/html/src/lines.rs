//! Своя строчная раскладка: разбиение абзаца на строки по правилам CSS.
//!
//! Переносчик GPUI знает одно правило — рвать по границам слов — и обойти его
//! снаружи можно лишь подсказками (нулевой пробел, словосоединитель). Этого
//! хватает на простые случаи и НЕ хватает на те, где ширина и точка разрыва
//! связаны:
//!
//! * `white-space: pre-wrap` — пробел в конце строки ВИСИТ за краем: место
//!   занимает, а перенос не вызывает;
//! * `white-space: break-spaces` — тот же пробел место занимает И даёт точку
//!   разрыва после себя;
//! * `overflow-wrap: break-word` — слово рвётся ТОЛЬКО если иначе не влезает;
//! * `line-break: anywhere` — разрыв где угодно, поверх запретов типографики;
//! * двунаправленный текст — знаки набираются в логическом порядке, а на экран
//!   идут в видимом, причём переставлять надо ГОТОВЫЕ прогоны, иначе рвётся
//!   арабская вязь.
//!
//! Поэтому строки считаются здесь: один раз меряется вся строка (`layout_line`
//! даёт положение каждого знака), по мере накопления ширины выбираются точки
//! разрыва, а на отрисовке каждая строка набирается своим `shape_line` и
//! рисуется на своём месте.

pub mod tabs;

mod atom_placement;
mod content_baselines;
mod controlled_shape;
mod ruby_justification;
mod selection_geometry;
mod text_raster_origin;
mod vertical_content_baselines;
mod vertical_geometry;
mod vertical_inline;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, SharedString, TextRun, Window, point, px, size,
};

/// Правила переноса, собранные из CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrap {
    /// `white-space: nowrap`/`pre` — мягких переносов нет вовсе.
    pub nowrap: bool,
    /// `white-space: break-spaces` — пробел занимает место и даёт разрыв.
    pub break_spaces: bool,
    /// `word-break: break-all` — разрыв между любыми знаками слова.
    pub break_all: bool,
    /// `line-break: anywhere` — разрыв где угодно, поверх запретов.
    pub anywhere: bool,
    /// `word-break: keep-all` — иероглифы не рвутся, только по пробелам.
    pub keep_all: bool,
    /// `overflow-wrap: break-word` — рвать слово, только если иначе не влезает.
    pub break_word: bool,
    /// `overflow-wrap: anywhere` — рвёт слово И при подсчёте размера по
    /// минимальному содержимому, в отличие от `break-word`.
    pub wrap_anywhere: bool,
    /// `direction: rtl` — основное направление абзаца.
    pub rtl: bool,
    /// `text-wrap: balance` — строки абзаца выравниваются по длине.
    pub balance: bool,
    /// `white-space: pre*` — пробелы сохраняются, в начале строки не срезаются.
    pub keep_spaces: bool,
    /// `line-break: normal` (1) / `loose` (2) — послабления UAX-14 для CJK
    /// (css-text-3 §5.2): разрыв перед малой каной и знаком долготы (класс
    /// CJ), в `loose` ещё и перед знаками повтора и между `…`.
    pub loose: u8,
    /// Язык куска — китайский или японский: часть послаблений §5.2 действует
    /// только «if the writing system is Chinese or Japanese».
    pub cjk_lang: bool,
}

/// Куда прижимать строку.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Мягкий перенос: разрешение разорвать слово, своей ширины он не имеет.
const SOFT_HYPHEN: char = '\u{00ad}';

/// Знак обрыва строки по `line-clamp`.
const ELLIPSIS: &str = "…";

/// Абзац со своей раскладкой строк.
pub struct Paragraph {
    text: SharedString,
    runs: Vec<TextRun>,
    font_size: Pixels,
    line_height: Pixels,
    align: Align,
    /// Выключка последней строки (`text-align-last`), если задана.
    align_last: Option<Align>,
    ruby_justify: bool,
    ruby_unit: bool,
    /// `unicode-bidi: plaintext` — сторона письма выбирается для КАЖДОГО
    /// абзаца между жёсткими разрывами по его первому сильному знаку. В
    /// преформате такой абзац — это строка, поэтому и `start`/`end` у каждой
    /// строки свои (HTML ставит это правило на `dir="auto"`).
    plaintext: Option<crate::computed::TextAlign>,
    /// Строки в ОБРАТНОМ порядке (снизу вверх): у `vertical-lr` колонки идут
    /// слева направо, а поворот по часовой кладёт ПЕРВУЮ строку правой —
    /// подача снизу вверх возвращает ей левую колонку.
    lines_reversed: bool,
    /// `line-clamp`: сколько строк показывать, остальные обрываются.
    clamp: Option<usize>,
    /// Знак обрыва положен, даже если абзац влез в предел целиком: точка
    /// среза стоит СРАЗУ ЗА ним, между блоками (css-overflow-4 §5.3 —
    /// знак привязан к точке среза, а не к тому, что абзац не поместился).
    /// Только авто-режим; счётный путь сюда не заходит.
    clamp_force: bool,
    /// `text-overflow: ellipsis` контейнера с обрезкой.
    text_overflow: bool,
    /// Маркер обрезки вместо многоточия (`text-overflow: <string>`).
    overflow_marker: Option<String>,
    /// Шрифт маркера: стиль БЛОКА-контейнера, не прогона у среза
    /// (css-overflow-4 §5) — иначе «123» набиралось Ahem-квадратами
    /// шрифта обрезанного куска.
    marker_font: Option<gpui::Font>,
    /// Кегль маркера: тоже БЛОЧНЫЙ. Базовый кегль абзаца — это `biggest`,
    /// кегль САМОГО КРУПНОГО куска; снятие собственного кегля прогона
    /// (`run.font_size = None`) отдаёт маркеру именно его, а не кегль
    /// блока, и строка-замена внутри `<span style="font-size:30px">`
    /// мерилась втрое шире нужного (`text-overflow-string-003…026`).
    marker_size: Option<Pixels>,
    /// Цвет знака обрыва — цвет БЛОКА (css-overflow-4 §5.3: знак — анонимный
    /// строчный ребёнок блока, а не куска у среза; `block-ellipsis-005`).
    marker_color: Option<Hsla>,
    /// `text-fit`: подбор кегля под ширину коробки.
    fit: Option<crate::computed::TextFit>,
    /// Масштабируемые части подбора кегля (css-text-5 §text-fit): интервалы
    /// в ДОЛЯХ кегля масштабируются вместе с ним, в точках и `em` — нет
    /// (`em` считается от вычисленного кегля, а его подбор не трогает).
    fit_spacing_scalable: bool,
    /// Заданная `line-height` (длина) при подборе не меняется; `normal` и
    /// число — считаются от использованного кегля и растут с ним.
    fit_line_height_fixed: bool,
    /// Шаг позиций табуляции (`tab-size` в точках).
    tab_stop: tabs::TabStops,
    /// Чем показывать перенос слова (`hyphenate-character`).
    hyphen: SharedString,
    /// Ширина этого знака — считается при раскладке, где есть окно.
    hyphen_w: std::cell::Cell<Pixels>,
    /// Куски ВНЕ потока: байтовое место в тексте → элемент. Рисуются поверх
    /// строк, места в них не занимают.
    /// Третье поле — блочный уровень: коробка встаёт в начало СЛЕДУЮЩЕЙ
    /// строки (см. `inline::Piece::Overlay`).
    overlays: Vec<(usize, AnyElement, crate::inline::OverlayAt)>,
    /// Трекинг (`letter-spacing`): добавка к каждому знаку.
    letter_spacing: Pixels,
    /// `word-spacing` — добавка к КАЖДОМУ пробелу. Шейпер о ней не знает,
    /// поэтому она добавляется к положению знака: сколько пробелов позади,
    /// столько добавок.
    word_spacing: Pixels,
    /// Вертикальное письмо: строка идёт СВЕРХУ ВНИЗ, а строки набегают по
    /// горизонтали. Абзац при этом остаётся обычным элементом раскладки —
    /// ограничение приходит от родителя по нужной оси, а не выдумывается.
    vertical: bool,
    /// `vertical-rl` — строки набегают справа налево.
    vertical_rl: bool,
    /// Dominant baseline in the rotated frame; sideways uses the real alphabetic baseline.
    vertical_central_baseline: bool,
    vertical_ccw: bool,
    selection_vertical: Option<(Bounds<Pixels>, bool)>,
    vertical_layout_origin: Point<Pixels>,
    /// Exact (unsnapped) layout origin minus the snapped paint origin. Box
    /// edges are snapped to device pixels, glyphs are not (Chromium paints
    /// text at its LayoutUnit position): the paragraph hands this to
    /// `Window::replace_glyph_offset` while painting its lines.
    glyph_nudge: Point<Pixels>,
    /// Exact (unsnapped) inline size minus the snapped one: alignment
    /// (`text-align: right/center`, rtl start) is measured from the exact
    /// edges, so a right-aligned glyph ends on the box's exact right edge.
    width_nudge: Pixels,
    /// Exact (unsnapped) inline size of the box at paint time: the basis of
    /// a percentage `text-indent` (css-text-3 §8.1: percentage of the
    /// containing block's inline size). The paint-time line limit is the
    /// snapped size plus one device pixel of slack, so a 50% indent landed
    /// half a device pixel off (`text-indent-103`).
    indent_basis: Option<Pixels>,
    vertical_inline: Option<(crate::computed::orthogonal::InlineConstraint, Option<crate::computed::orthogonal::InlineKeyword>)>,
    /// Предел строки для ОРТОГОНАЛЬНОГО потока: ось строки абзаца совпала с
    /// осью потока родителя, а та не ограничена. По CSS Writing Modes §7.3
    /// предел берётся от ближайшего предка-контейнера прокрутки, а при его
    /// отсутствии — от начального содержащего блока, то есть от окна.
    ortho_limit: Option<Pixels>,
    /// Какая пунктуация свисает за край (`hanging-punctuation`).
    hanging: crate::computed::Hanging,
    /// Отступ первой строки (`text-indent`).
    indent: Indent,
    /// Места знаков-распорок (`inline::SPACER`) — байтовые смещения по
    /// возрастанию. Точки переноса считаются по тексту без них.
    spacers: Vec<usize>,
    /// Вырезы обтекания (`shape-outside`): формы слева и справа, в
    /// координатах от верха абзаца. Сужают СВОИ строки по их высоте.
    flow: std::sync::Arc<(Vec<crate::flow::FloatShape>, Vec<crate::flow::FloatShape>)>,
    /// Опознание абзаца для памяти выделения. Без него абзац не выделяется:
    /// состояние между кадрами хранит раскладка по этому ключу.
    id: Option<ElementId>,
    /// Цвет подложки выделенного куска.
    highlight: Hsla,
    wrap: Wrap,
    /// Правила переноса ПО КУСКАМ: `word-break` или `overflow-wrap`, заданные
    /// на вложенном `<span>`, действуют только на его байты. Пусто — значит
    /// весь абзац живёт по одному правилу.
    spans: Vec<(std::ops::Range<usize>, Wrap)>,
    /// Межсловный интервал ПО КУСКАМ: `word-spacing` на вложенном `<span>`
    /// действует только на пробелы внутри него. Пусто — значит на весь абзац
    /// один интервал.
    word_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Трекинг ПО КУСКАМ: `letter-spacing` на вложенном `<span>` действует
    /// только на его знаки. Набор принимает трекинг скаляром, поэтому разница
    /// с общим значением добавляется к положению знака, а слово набирается
    /// своим трекингом.
    letter_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Сдвиг куска по вертикали (`vertical-align: super`/`sub`): смещение
    /// базовой линии в точках, вниз положительное.
    shift_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Своя `line-height` куска (§10.8): строка растёт до полулидинга самого
    /// высокого куска. Отдельно от `line_height` — та принадлежит блоку.
    lh_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Относительный сдвиг кусков (CSS 2.1 §9.4.3): двигает только
    /// отрисовку. Отдельно от `shift_spans` — тот растит строчную коробку,
    /// а этот на поток не влияет вовсе.
    rel_spans: Vec<(std::ops::Range<usize>, (f32, f32))>,
    /// Атомарные строчные коробки В СТРОКЕ (CSS 2.1 §9.2.2, §10.8): место в
    /// тексте держит распорка (U+FEFF), её продвижение — ширина атома, а сам
    /// атом раскладывается отдельно и ставится на базовую линию своей строки.
    atoms: Vec<AtomSlot>,
    /// Замеры атомов (ширина, высота, базовая линия) — копируются в щуп
    /// замера, сами элементы туда не уходят.
    atom_boxes: Vec<AtomBox>,
    /// Метрики струта абзаца (§10.8.1): подъём, спуск и x-высота первого
    /// прогона — от них считается, насколько атом вылезает за строку.
    strut: (f32, f32, f32),
    /// Подъём и спуск ОСНОВНОГО шрифта каждого прогона: по ним строка с
    /// кусками разного кегля ставит их на ОДНУ базовую линию (§10.8) — так же,
    /// как сплошной набор строки (`ShapedLine::paint` берёт наибольшие подъём
    /// и спуск строки).
    run_metrics: Vec<(f32, f32)>,
    /// Куски с `vertical-align: top`/`bottom` (§10.8.1): отрезок байт, край
    /// (`true` — верх) и высота их строчной коробки (`line-height` куска).
    /// Равняются по краю ГОТОВОЙ строки и растят её, только если выше неё.
    edge_spans: Vec<(std::ops::Range<usize>, bool, f32)>,
    /// `text-box-trim` блока (начало, конец): аннотация руби и знак акцента
    /// первой строки сверху и последней снизу её не растят — срез идёт по краю текста
    /// корневой строчной коробки, и выход аннотации срезался бы всё равно
    /// (css-inline-3 §text-box-trim, `text-box-trim-ruby-start-001`).
    ruby_trim: (bool, bool),
    /// Куски со знаком акцента (`text-emphasis`): (отрезок, снизу?, высота
    /// знака). Знак растит строку так же, как аннотация руби
    /// (css-text-decor-3 §5.3 «If emphasis marks … do not fit, the UA must
    /// increase the line height»): эталоны семьи `text-emphasis-line-height-*`
    /// строятся именно из руби.
    emph_spans: Vec<(std::ops::Range<usize>, bool, f32)>,
    /// Строчные коробки кусков ПОСТРОЧНО (CSS 2.1 §10.8.1): отрезок байт →
    /// `line-height` куска в точках. Непусто — у абзаца куски разного кегля,
    /// гарнитуры или высоты строки, и `line_height` абзаца — это СТРУТ блока,
    /// а каждая строка собирается из коробок своих кусков: верх — наибольший
    /// подъём с полулидингом, низ — наибольший спуск. Прежде на весь абзац
    /// шла одна высота по самому крупному куску.
    box_spans: Vec<(std::ops::Range<usize>, f32)>,
    /// Шрифт и кегль струта (шрифт самого блока) — для `box_spans`.
    strut_run: Option<(gpui::Font, Pixels)>,
    /// Подъём и спуск струта в точках (меряются в `request_layout`).
    strut_box: (f32, f32),
    /// Границы строк в байтах — считаются на замере, переиспользуются на
    /// отрисовке.
    lines: Vec<Line>,
}

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
struct AtomSlot {
    at: usize,
    el: AnyElement,
    align: AtomAlign,
    probe: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
    /// Узел самой обёртки (см. `LayoutTap`).
    root: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
    /// Узлы уровней аннотаций руби (`ruby_extent`): `true` — под базой.
    extents: RubyExtents,
    /// Атом за последней строкой (оборван `line-clamp`): не ставится и не
    /// рисуется.
    hidden: bool,
}

/// Узлы стопок аннотаций одного руби: (под базой?, полулидинг базы, узел).
/// Полулидинг вычитается: стопка стоит на краю коробки строки базы, а
/// аннотация в браузере — на краю её СОДЕРЖИМОГО.
pub type RubyExtents = Vec<(bool, f32, std::rc::Rc<std::cell::Cell<Option<LayoutId>>>)>;

thread_local! {
    /// Сбор узлов аннотаций для атома, который сейчас строится
    /// (`collect_ruby_extents`). `None` — сбора нет: руби вне строки абзаца
    /// своих аннотаций никому не отдаёт.
    static RUBY_EXTENTS: std::cell::RefCell<Option<RubyExtents>> =
        const { std::cell::RefCell::new(None) };
}

/// Построить атом, собрав узлы аннотаций руби, которые он заведёт.
/// Вложенный сбор (атом внутри атома) своё забирает сам: прежний список
/// восстанавливается после вызова.
pub fn collect_ruby_extents<T>(build: impl FnOnce() -> T) -> (T, RubyExtents) {
    let saved = RUBY_EXTENTS.with(|r| r.replace(Some(Vec::new())));
    let out = build();
    let mine = RUBY_EXTENTS.with(|r| r.replace(saved)).unwrap_or_default();
    (out, mine)
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
            .map(|v| v.push((under, inset, slot.clone())))
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
struct AtomBox {
    at: usize,
    h: f32,
    base: f32,
    align: AtomAlign,
    /// Аннотации руби над и под коробкой (`ruby_extent`).
    over: f32,
    under: f32,
}

/// Щуп базовой линии атома: пустой лист с базовой линией на своём верху.
/// В ряду `align-items: baseline` рядом с атомом его верх встаёт ровно на
/// базовую линию атома, и раскладка отдаёт её положением щупа. Для атома без
/// базовой линии taffy берёт нижний край полей (`flexbox.rs`) — у замещаемого
/// это и есть его базовая по §10.8.1.
struct BaselineProbe {
    slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
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
struct LayoutTap {
    child: AnyElement,
    slot: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
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

/// Строка: что рисовать и сколько она занимает.
#[derive(Clone, Debug)]
struct Line {
    /// Байтовый отрезок исходного текста.
    range: std::ops::Range<usize>,
    /// Ширина без хвостовых пробелов — по ней идёт выключка.
    width: Pixels,
    /// Строка оборвана `line-clamp`: за её текстом рисуется многоточие.
    ellipsis: bool,
    /// Строка кончилась мягким переносом: за ней рисуется знак переноса.
    hyphen: bool,
    /// Отступ строки (`text-indent`). Отрицательный выводит строку за край.
    indent: Pixels,
}

/// `text-indent`: отступ первой строки блока.
///
/// Значение хранится ДВУМЯ частями. Абсолютную (`px`) считает разбор стилей,
/// а доля (`pct`) берётся от ширины содержащего блока и известна только там,
/// где ширина уже решена, — в раскладке строк.
#[derive(Default, Clone, Copy, PartialEq)]
pub struct Indent {
    /// Абсолютная часть в точках; отрицательная выводит строку за край.
    pub px: f32,
    /// Доля ширины строки: `10%` — это `0.1`.
    pub pct: f32,
    /// `each-line`: отступ повторяется после каждого жёсткого разрыва.
    pub each_line: bool,
    /// `hanging`: отступ получают все строки, КРОМЕ той, что получила бы его.
    pub hanging: bool,
}

impl Paragraph {
    pub fn new(
        text: SharedString,
        runs: Vec<TextRun>,
        font_size: Pixels,
        line_height: Pixels,
        align: Align,
        wrap: Wrap,
    ) -> Self {
        Paragraph {
            text,
            runs,
            font_size,
            line_height,
            align,
            align_last: None,
            ruby_justify: false,
            ruby_unit: false,
            plaintext: None,
            lines_reversed: false,
            letter_spacing: px(0.),
            word_spacing: px(0.),
            vertical: false,
            vertical_rl: false,
            vertical_central_baseline: true,
            vertical_ccw: false,
            selection_vertical: None,
            vertical_layout_origin: point(px(0.0), px(0.0)),
            glyph_nudge: point(px(0.0), px(0.0)),
            width_nudge: px(0.0),
            indent_basis: None,
            vertical_inline: None,
            ortho_limit: None,
            hanging: crate::computed::Hanging::default(),
            indent: Indent::default(),
            spacers: Vec::new(),
            flow: std::sync::Arc::new((Vec::new(), Vec::new())),
            id: None,
            highlight: Hsla::default(),
            wrap,
            spans: Vec::new(),
            word_spans: Vec::new(),
            letter_spans: Vec::new(),
            shift_spans: Vec::new(),
            lh_spans: Vec::new(),
            rel_spans: Vec::new(),
            atoms: Vec::new(),
            atom_boxes: Vec::new(),
            strut: (0.0, 0.0, 0.0),
            run_metrics: Vec::new(),
            edge_spans: Vec::new(),
            ruby_trim: (false, false),
            emph_spans: Vec::new(),
            box_spans: Vec::new(),
            strut_run: None,
            strut_box: (0.0, 0.0),
            lines: Vec::new(),
            clamp: None,
            clamp_force: false,
            text_overflow: false,
            overflow_marker: None,
            marker_font: None,
            marker_size: None,
            marker_color: None,
            fit: None,
            fit_spacing_scalable: true,
            fit_line_height_fixed: false,
            tab_stop: tabs::TabStops::uniform(64.0),
            hyphen: SharedString::from("\u{2010}"),
            hyphen_w: std::cell::Cell::new(px(0.)),
            overlays: Vec::new(),
        }
    }

    /// Пустой абзац — только чтобы на миг занять место настоящего, пока тот
    /// рисуется в повёрнутой системе координат.
    fn empty() -> Self {
        Paragraph::new(
            SharedString::default(),
            Vec::new(),
            px(0.),
            px(0.),
            Align::Left,
            Wrap::default(),
        )
    }

    /// Сдвиг кусков по вертикали: отрезок байт → смещение базовой линии.
    pub fn rel_spans(mut self, spans: Vec<(std::ops::Range<usize>, (f32, f32))>) -> Self {
        self.rel_spans = spans;
        self
    }

    pub fn lh_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.lh_spans = spans;
        self
    }

    /// Строчные коробки кусков и шрифт струта (см. поле `box_spans`).
    pub fn line_boxes(
        mut self,
        spans: Vec<(std::ops::Range<usize>, f32)>,
        strut: Option<(gpui::Font, Pixels)>,
    ) -> Self {
        self.box_spans = spans;
        self.strut_run = strut;
        self
    }

    /// Куски со знаком акцента (см. поле `emph_spans`).
    pub fn emph_spans(mut self, spans: Vec<(std::ops::Range<usize>, bool, f32)>) -> Self {
        self.emph_spans = spans;
        self
    }

    /// `text-box-trim` блока (см. поле `ruby_trim`).
    pub fn ruby_trim(mut self, start: bool, end: bool) -> Self {
        self.ruby_trim = (start, end);
        self
    }

    /// Куски, прижатые к краю строки (см. поле `edge_spans`).
    pub fn edge_spans(mut self, spans: Vec<(std::ops::Range<usize>, bool, f32)>) -> Self {
        self.edge_spans = spans;
        self
    }

    pub fn shift_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.shift_spans = spans;
        self
    }

    /// Трекинг по кускам: отрезок байт → своё значение.
    pub fn letter_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.letter_spans = spans;
        self
    }

    /// Межсловный интервал по кускам: отрезок байт → своя добавка.
    pub fn word_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.word_spans = spans;
        self
    }

    /// Правила переноса по кускам: отрезок байт → своё правило.
    pub fn spans(mut self, spans: Vec<(std::ops::Range<usize>, Wrap)>) -> Self {
        self.spans = spans;
        self
    }

    /// Выключка последней строки — своя, если разметка её задала.
    /// `unicode-bidi: plaintext`: логическая выключка, которую надо решать по
    /// стороне КАЖДОЙ строки.
    pub fn plaintext(mut self, align: Option<crate::computed::TextAlign>) -> Self {
        self.plaintext = align;
        self
    }

    /// Рисовать строки снизу вверх (см. поле `lines_reversed`).
    pub fn reversed_lines(mut self, on: bool) -> Self {
        self.lines_reversed = on;
        self
    }

    pub fn align_last(mut self, align: Option<Align>) -> Self {
        self.align_last = align;
        self
    }

    /// Трекинг: добавка к каждому знаку (`letter-spacing`).
    pub fn letter_spacing(mut self, extra: Pixels) -> Self {
        self.letter_spacing = extra;
        self
    }

    pub fn word_spacing(mut self, extra: Pixels) -> Self {
        self.word_spacing = extra;
        self
    }

    /// Предел ортогонального потока (см. поле `ortho_limit`).
    pub fn ortho_limit(mut self, limit: Option<Pixels>) -> Self {
        self.ortho_limit = limit;
        self
    }

    /// Вертикальное письмо и сторона набегания строк.
    pub fn vertical(mut self, on: bool, rl: bool) -> Self {
        self.vertical = on;
        self.vertical_rl = rl;
        self
    }

    /// Разрешить выделение мышью: абзац заводит своё состояние и область
    /// попадания.
    pub fn selectable(mut self, id: ElementId, highlight: Hsla) -> Self {
        self.id = Some(id);
        self.highlight = highlight;
        self
    }

    /// Свисающая пунктуация (`hanging-punctuation`).
    pub fn hanging(mut self, hanging: Option<crate::computed::Hanging>) -> Self {
        self.hanging = hanging.unwrap_or_default();
        self
    }

    /// Места знаков-распорок строчных коробок.
    pub fn spacers(mut self, spacers: Vec<usize>) -> Self {
        self.spacers = spacers;
        self
    }

    /// Вырезы обтекания (`shape-outside`).
    pub fn flow_shapes(
        mut self,
        flow: std::sync::Arc<(Vec<crate::flow::FloatShape>, Vec<crate::flow::FloatShape>)>,
    ) -> Self {
        self.flow = flow;
        self
    }

    /// Расходится ли трекинг ОТРЕЗКОВ с общим трекингом абзаца.
    ///
    /// `letter_spans` заполняется на КАЖДЫЙ кусок с заданным `letter-spacing`,
    /// а свойство наследуется: `p { letter-spacing: 1em }` даёт запись на все
    /// куски с тем же значением, что и общее. Пустой разницы достаточно, чтобы
    /// абзац ушёл на пословную краску, где видимого порядка UAX#9 нет вовсе —
    /// и буквы вставали в логическом порядке (`bidi-005b`…`-009b`).
    ///
    /// Незримый знак (распорка полей, метка атома, управление
    /// двунаправленностью) несёт свой трекинг всегда: у него он и есть
    /// продвижение, поэтому расхождением считается любое НЕнулевое значение.
    fn letter_spans_diverge(&self) -> bool {
        let common = f32::from(self.letter_spacing);
        self.letter_spans.iter().any(|(r, v)| {
            let seen = f32::from(*v);
            let body = self.text.get(r.clone()).unwrap_or("");
            let invisible = !body.is_empty()
                && body.chars().all(|c| {
                    matches!(
                        c,
                        '\u{feff}' | '\u{200b}' | '\u{200e}' | '\u{200f}'
                            | '\u{202a}'..='\u{202e}'
                            | '\u{2066}'..='\u{2069}'
                    )
                });
            // Незримый знак с ОБЩИМ трекингом — просто унаследовавший его
            // знак управления двунаправленностью: расхождением он не является.
            // Расходится только распорка, чей трекинг и есть её ширина.
            if invisible && (seen - common).abs() <= 0.01 {
                return false;
            }
            if invisible {
                seen != 0.0
            } else {
                (seen - common).abs() > 0.01
            }
        })
    }

    /// Вырез строки номер `line_no`: (слева, справа).
    /// Надбавки строки сверху и снизу от сдвинутых по вертикали кусков.
    ///
    /// Сдвиг `vertical-align` не просто двигает знаки — он РАСТИТ строчную
    /// коробку (CSS 2.1 §10.8): её верх и низ берутся по объединению всех
    /// кусков после выравнивания. На каждую строку своя пара: абзац с
    /// надстрочным знаком в одной строке не должен раздувать остальные.
    fn line_padding(&self) -> Vec<(f32, f32)> {
        if self.shift_spans.is_empty()
            && self.lh_spans.is_empty()
            && self.atom_boxes.is_empty()
            && self.edge_spans.is_empty()
            && self.box_spans.is_empty()
            && self.emph_spans.is_empty()
        {
            return vec![(0.0, 0.0); self.lines.len()];
        }
        let lh = f32::from(self.line_height);
        let last_line = self.lines.len().saturating_sub(1);
        self.lines
            .iter()
            .enumerate()
            .map(|(line_no, line)| {
                let (mut above, mut below) = (0.0f32, 0.0f32);
                let boxes = !self.box_spans.is_empty() && self.run_metrics.len() == self.runs.len();
                if boxes {
                    let (top, bot) = self.line_extents(&line.range);
                    let a = self.line_base(&line.range);
                    above = top - a;
                    below = bot - (lh - a);
                }
                // Кусок со своей `line-height` растит строку симметрично:
                // полулидинг его коробки отступа ложится сверху и снизу
                // (§10.8). Блочное значение уже учтено высотой строки.
                for (range, lh) in self.lh_spans.iter().filter(|_| !boxes) {
                    if range.end <= line.range.start || range.start >= line.range.end {
                        continue;
                    }
                    let half = (f32::from(*lh) - f32::from(self.line_height)) / 2.0;
                    if half > 0.0 {
                        above = above.max(half);
                        below = below.max(half);
                    }
                }
                for (range, dy) in self.shift_spans.iter().filter(|_| !boxes) {
                    if range.end <= line.range.start || range.start >= line.range.end {
                        continue;
                    }
                    // Ось сдвига смотрит вниз: отрицательное поднимает знак
                    // над строкой, положительное опускает.
                    let v = f32::from(*dy);
                    above = above.max(-v);
                    below = below.max(v);
                }
                // Атом растит строку на то, чем его коробка полей выходит за
                // струт (§10.8: строчная коробка — от верха самой высокой
                // коробки до низа самой низкой). `top`/`bottom` решаются
                // ПОСЛЕ остальных: они равняются по уже собранной строке и
                // растят её, только если сами выше (§10.8.1).
                let inside = |at: usize| at >= line.range.start && at < line.range.end;
                let a = self.line_base(&line.range);
                for b in self.atom_boxes.iter().filter(|b| inside(b.at)) {
                    if matches!(b.align, AtomAlign::Top | AtomAlign::Bottom) {
                        continue;
                    }
                    let t = self.atom_top(b);
                    // Аннотация руби растит строку, только выходя за неё:
                    // полулидинг строки она занимает даром (css-ruby-1 §3.4).
                    let over = if line_no == 0 && self.ruby_trim.0 { 0.0 } else { b.over };
                    let under = if line_no == last_line && self.ruby_trim.1 {
                        0.0
                    } else {
                        b.under
                    };
                    above = above.max(-(t - over) - a);
                    below = below.max(t + b.h + under - (lh - a));
                }
                // Знак акцента стоит над (под) коробкой содержимого своего
                // прогона и растит строку, только выходя за неё.
                for (range, under, h) in &self.emph_spans {
                    if range.end <= line.range.start || range.start >= line.range.end {
                        continue;
                    }
                    let mut at = 0usize;
                    let metrics = self.runs.iter().zip(&self.run_metrics).find_map(|(run, m)| {
                        let s = at;
                        at += run.len;
                        (range.start >= s && range.start < at).then_some(*m)
                    });
                    let Some((ra, rd)) = metrics else { continue };
                    // Срез текстовой коробки знак не растит так же, как
                    // аннотацию (`text-box-trim-ruby-start-002`).
                    if (*under && line_no == last_line && self.ruby_trim.1)
                        || (!*under && line_no == 0 && self.ruby_trim.0)
                    {
                        continue;
                    }
                    if *under {
                        below = below.max(rd + h - (lh - a));
                    } else {
                        above = above.max(ra + h - a);
                    }
                }
                // Прижатые к краю — атомы и куски текста — после всех
                // остальных: строка растёт, только если такой кусок выше.
                let edges = self
                    .atom_boxes
                    .iter()
                    .filter(|b| inside(b.at))
                    .filter_map(|b| match b.align {
                        AtomAlign::Top => Some((true, b.h)),
                        AtomAlign::Bottom => Some((false, b.h)),
                        _ => None,
                    })
                    .chain(
                        self.edge_spans
                            .iter()
                            .filter(|(r, _, _)| {
                                r.start < line.range.end && r.end > line.range.start
                            })
                            .map(|(_, top, h)| (*top, *h)),
                    );
                for (top, h) in edges {
                    let total = lh + above + below;
                    if h <= total {
                        continue;
                    }
                    if top {
                        below += h - total;
                    } else {
                        above += h - total;
                    }
                }
                (above, below)
            })
            .collect()
    }

    fn flow_cut(&self, line_no: usize) -> (f32, f32) {
        if self.flow.0.is_empty() && self.flow.1.is_empty() {
            return (0.0, 0.0);
        }
        let lh = f32::from(self.line_height);
        let (y0, y1) = (line_no as f32 * lh, (line_no as f32 + 1.0) * lh);
        let l = self
            .flow
            .0
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        let r = self
            .flow
            .1
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        // След вырезов строк: FLOW_DBG=1.
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("FLOW_DBG").is_ok());
            *ON
        } {
            eprintln!(
                "FLOWCUT #{line_no} y={y0}..{y1} l={l} r={r} {:?}",
                self.flow
            );
        }
        (l, r)
    }

    /// Отступ первой строки (`text-indent`).
    pub fn indent(mut self, indent: Indent) -> Self {
        self.indent = indent;
        self
    }

    /// Отступ ЭТОЙ строки в точках.
    ///
    /// Доля считается от ширины строки (css-text-3 §7.1: процент берётся от
    /// ширины содержащего блока), поэтому предел приходит сюда: при замере по
    /// содержимому его нет, и доля обращается в ноль — как в браузере.
    fn indent_of(&self, head_of_part: bool, first_part: bool, limit: Option<Pixels>) -> Pixels {
        let own = if self.indent.each_line {
            head_of_part
        } else {
            head_of_part && first_part
        };
        if own == self.indent.hanging {
            return px(0.);
        }
        let basis = limit.map(|l| self.indent_basis.unwrap_or(l));
        let pct = self.indent.pct * f32::from(basis.unwrap_or(px(0.)));
        px(self.indent.px + pct)
    }

    /// Сколько байт в начале строки свисает за левый край.
    ///
    /// Свисает только открывающий знак и только в начале ПЕРВОЙ строки
    /// абзаца: место он занимает в поле, а не в колонке, поэтому в ширину
    /// строки не входит.
    fn hang_first(&self, start: usize) -> usize {
        if !self.hanging.first || start != 0 {
            return 0;
        }
        match self.text[start..].chars().next() {
            Some(ch) if is_opening(ch) => ch.len_utf8(),
            _ => 0,
        }
    }

    /// Сколько байт в конце строки свисает за правый край.
    fn hang_last(&self, end: usize, closing_line: bool, over: bool) -> usize {
        let Some(ch) = self.text[..end].chars().next_back() else {
            return 0;
        };
        let hangs = (self.hanging.last && closing_line && is_closing(ch))
            || (self.hanging.force_end && is_stop(ch))
            // `allow-end` свисает ТОЛЬКО когда строка иначе не влезает —
            // в отличие от `force-end`, который свисает всегда. Пока разницы
            // не было, строки рвались на знак позже, чем надо.
            || (self.hanging.allow_end && over && is_stop(ch));
        if hangs { ch.len_utf8() } else { 0 }
    }

    /// Куски между обязательными разрывами: набор не принимает перевод строки,
    /// поэтому мерить приходится по кускам, а положения знаков сшивать.
    ///
    /// Результат запоминается: раскладка спрашивает размер абзаца по многу раз
    /// за кадр (перебор ширин в гибком контейнере), а набор строки — самая
    /// дорогая операция здесь.
    fn measure(&self, window: &mut Window) -> Vec<Seg> {
        let key = self.measure_key();
        if let Some(hit) = MEASURED.with(|c| {
            c.borrow()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return hit;
        }
        let out = self.measure_uncached(window);
        MEASURED.with(|c| {
            let mut cache = c.borrow_mut();
            if cache.len() >= MEASURE_CACHE {
                cache.remove(0);
            }
            cache.push((key, out.clone()));
        });
        out
    }

    /// Ключ памяти замера: от чего зависит положение знаков.
    fn measure_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut h);
        f32::from(self.font_size).to_bits().hash(&mut h);
        f32::from(self.letter_spacing).to_bits().hash(&mut h);
        f32::from(self.word_spacing).to_bits().hash(&mut h);
        self.tab_stop.hash_into(&mut h);
        for run in &self.runs {
            run.len.hash(&mut h);
            run.font_size.map(|s| f32::from(s).to_bits()).hash(&mut h);
            run.font.family.hash(&mut h);
            run.font.weight.0.to_bits().hash(&mut h);
            (run.font.style as u8).hash(&mut h);
            // Возможности OpenType меняют и подстановку, и продвижение
            // (`vert`, `hwid`) — без них кэш отдавал чужой набор.
            for (tag, value) in run.font.features.tag_value_list() {
                tag.hash(&mut h);
                value.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Ключ РАЗРЕЗА: замер плюс всё, от чего зависит перенос и ширины строк.
    fn split_key(&self, limit: Option<Pixels>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.measure_key().hash(&mut h);
        limit.map(|l| f32::from(l).to_bits()).hash(&mut h);
        self.indent_basis.map(|l| f32::from(l).to_bits()).hash(&mut h);
        let w = &self.wrap;
        [
            w.nowrap,
            w.break_spaces,
            w.break_all,
            w.anywhere,
            w.keep_all,
            w.break_word,
            w.wrap_anywhere,
            w.rtl,
            w.balance,
            w.keep_spaces,
        ]
        .hash(&mut h);
        self.indent.px.to_bits().hash(&mut h);
        for f in self.flow.0.iter().chain(self.flow.1.iter()) {
            f.hash_bits().hash(&mut h);
        }
        self.indent.pct.to_bits().hash(&mut h);
        self.indent.each_line.hash(&mut h);
        self.indent.hanging.hash(&mut h);
        let hg = &self.hanging;
        [hg.first, hg.last, hg.force_end, hg.allow_end].hash(&mut h);
        self.clamp.hash(&mut h);
        self.clamp_force.hash(&mut h);
        self.text_overflow.hash(&mut h);
        self.overflow_marker.hash(&mut h);
        self.marker_size
            .map(|s| f32::from(s).to_bits())
            .hash(&mut h);
        self.hyphen.hash(&mut h);
        self.spacers.hash(&mut h);
        for (r, v) in self.word_spans.iter().chain(&self.letter_spans) {
            r.start.hash(&mut h);
            r.end.hash(&mut h);
            f32::from(*v).to_bits().hash(&mut h);
        }
        h.finish()
    }

    fn measure_uncached(&self, window: &mut Window) -> Vec<Seg> {
        let mut out = Vec::new();
        let mut start = 0usize;
        let mut offset = px(0.);
        loop {
            // Кусок кончается переводом строки ИЛИ табуляцией: табуляция — не
            // знак со своей шириной, а прыжок к следующей позиции табуляции, и
            // отдавать её набору нечего (`break-spaces-tab`).
            let end = self.text[start..]
                .find(['\n', '\t', SOFT_HYPHEN])
                .map(|i| start + i)
                .unwrap_or(self.text.len());
            let runs = slice_runs(&self.runs, &(start..end));
            let layout = window.text_system().layout_line_spaced(
                &self.text[start..end],
                self.font_size,
                &runs,
                None,
                self.letter_spacing,
            );
            // Ширина куска вместе с трекингом кусков и `word-spacing`: набор
            // их не знает, `x_at` добавляет их сам — и позиция табуляции за
            // куском обязана их учесть (`word-spacing-characters-001`:
            // табуляция после растянутых пробелов вставала раньше).
            let width = layout.width + self.seg_extra(start, end);
            out.push(Seg {
                start,
                end,
                layout,
                offset,
            });
            if end >= self.text.len() {
                break;
            }
            let mark = self.text[end..].chars().next().unwrap_or('\n');
            match mark {
                '\n' => offset = px(0.),
                // Мягкий перенос своей ширины не имеет: он лишь ПОЗВОЛЯЕТ
                // разрыв. Пока он доезжал до набора, шрифт давал ему ширину
                // дефиса, и слово рвалось раньше времени (`hyphens-manual-011`:
                // «Deoxy-ribo-» вместо «Deoxyribo-»).
                SOFT_HYPHEN => offset += width,
                _ => {
                    let x = f32::from(offset + width);
                    let next = self.tab_stop.next(end, x);
                    offset = px(next);
                }
            }
            start = end + mark.len_utf8();
        }
        out
    }

    /// Ширина отрезка строки.
    ///
    /// Положения знаков считаются от начала своего куска, поэтому границу
    /// надо толковать по её роли: КОНЕЦ отрезка сразу за переводом строки —
    /// это конец прошлого куска, а НАЧАЛО с тем же индексом — начало нового.
    /// Иначе строка, открывающая новый кусок, получала ширину со знаком минус
    /// и уезжала за край коробки.
    fn span(&self, segs: &[Seg], from: usize, to: usize) -> Pixels {
        if to <= from {
            return px(0.);
        }
        let width = self.x_at(segs, to, Edge::End) - self.x_at(segs, from, Edge::Start);
        if width < px(0.) { px(0.) } else { width }
    }

    /// Положение знака от начала своего куска.
    /// Добавка к ширине набранного куска `start..end`: трекинг кусков сверх
    /// общего и `word-spacing` у пробелов (то же, что `x_at` прибавляет к
    /// положению знака в конце куска).
    fn seg_extra(&self, start: usize, end: usize) -> Pixels {
        let mut extra = px(0.);
        if self.letter_spans.is_empty() && self.word_spacing == px(0.) && self.word_spans.is_empty()
        {
            return extra;
        }
        for (off, ch) in self.text[start..end].char_indices() {
            let at = start + off;
            if let Some((_, v)) = self.letter_spans.iter().find(|(r, _)| r.contains(&at)) {
                extra += *v - self.letter_spacing;
            }
            if word_separator(ch) {
                extra += self
                    .word_spans
                    .iter()
                    .find(|(r, _)| r.contains(&at))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.word_spacing);
            }
        }
        extra
    }

    fn x_at(&self, segs: &[Seg], i: usize, edge: Edge) -> Pixels {
        let i = i.min(self.text.len());
        let after_break = edge == Edge::End && i > 0 && self.text.as_bytes()[i - 1] == b'\n';
        let seg = if after_break {
            segs.iter().find(|s| s.end + 1 == i)
        } else {
            segs.iter().find(|s| i <= s.end)
        };
        let Some(seg) = seg.or_else(|| segs.last()) else {
            return px(0.);
        };
        let base = if i >= seg.end {
            seg.layout.width
        } else if i <= seg.start {
            px(0.)
        } else {
            seg.layout.x_for_index(i - seg.start)
        };
        // Сдвиг куска внутри строки: его задаёт табуляция перед ним.
        let mut base = base + seg.offset;
        if !self.letter_spans.is_empty() {
            let upto = i.min(seg.end);
            for (off, _) in self.text[seg.start..upto].char_indices() {
                let at = seg.start + off;
                if let Some((_, v)) = self.letter_spans.iter().find(|(r, _)| r.contains(&at)) {
                    base += *v - self.letter_spacing;
                }
            }
        }
        if self.word_spacing == px(0.) && self.word_spans.is_empty() {
            return base;
        }
        // Набор про `word-spacing` не знает: знак сдвинут на столько добавок,
        // сколько пробелов осталось позади него внутри куска. Добавка у
        // каждого пробела СВОЯ — заданная на том куске, в который он попал.
        let upto = i.min(seg.end);
        let mut extra = px(0.);
        for (off, _) in self.text[seg.start..upto]
            .char_indices()
            .filter(|(_, c)| word_separator(*c))
        {
            let at = seg.start + off;
            extra += self
                .word_spans
                .iter()
                .find(|(r, _)| r.contains(&at))
                .map(|(_, v)| *v)
                .unwrap_or(self.word_spacing);
        }
        base + extra
    }

    /// Ширина по минимальному содержимому — самый широкий кусок, который
    /// разорвать нельзя.
    ///
    /// Нужна раскладке: под неё она меряет высоту, когда ширина ещё не
    /// решена. Ноль тут не годится — по нулю строка рвётся на каждом знаке, и
    /// коробка выходит во много раз выше настоящей.
    fn min_content(&self, window: &mut Window) -> Pixels {
        let segs = self.measure(window);
        let mut best = px(0.);
        let mut start = 0usize;
        let mut chunk = |from: usize, to: usize, this: &Self| {
            let end = if this.spaces_are_content() {
                to
            } else {
                this.hang_tail(from, to)
            };
            // Трекинг ПОСЛЕДНЕГО знака куска на конце строки не действует
            // (css-text-3 §8.2) — `lay_in` его вычитает, а минимум по
            // содержимому считал, и кусок выходил шире на `letter-spacing`.
            let w = this.span(&segs, from, end) - this.tail_spacing(end);
            let w = if w < px(0.) { px(0.) } else { w };
            if w > best {
                best = w;
            }
        };
        // `overflow-wrap: anywhere` — единственное из семейства, что меняет
        // размер по минимальному содержимому: слово рвётся и здесь, поэтому
        // точками счёта становятся ВСЕ границы знаков (css-text-3 §5.5).
        let stops: Vec<Stop> = if self.wrap.wrap_anywhere {
            self.text
                .char_indices()
                .skip(1)
                .map(|(at, _)| Stop {
                    at,
                    mandatory: false,
                })
                .collect()
        } else {
            // Куски со своим `overflow-wrap: anywhere` добавляют границы
            // знаков только внутри себя.
            let mut stops = self.opportunities();
            for (range, w) in &self.spans {
                if !w.wrap_anywhere {
                    continue;
                }
                for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                    stops.push(Stop {
                        at: range.start + i,
                        mandatory: false,
                    });
                }
            }
            stops.sort_by_key(|s| (s.at, !s.mandatory));
            stops.dedup_by_key(|s| s.at);
            stops
        };
        for stop in stops {
            if stop.at <= start {
                continue;
            }
            chunk(start, stop.at, self);
            start = stop.at;
        }
        chunk(start, self.text.len(), self);
        best
    }

    /// Разбить текст на строки под заданную ширину и оборвать по `line-clamp`.
    ///
    /// Порядок важен: сперва обрыв, потом выравнивание длин. Выровнять надо
    /// то, что ОСТАЛОСЬ видимым, и с учётом места, отнятого многоточием
    /// (`text-wrap-balance-line-clamp-003`).
    /// Разрез с памятью: раскладка гоняет его по 3-5 раз на абзац за кадр
    /// (min/max/definite у гибкого родителя + подготовка), а разрез — самое
    /// дорогое место резчика. Ключ обязан покрывать ВСЁ, что читает
    /// `split_uncached`, иначе устаревшие переносы сдвинут пиксели.
    fn split(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        // Ширина знака переноса взводится ДО обращения в память: при
        // попадании она нужна отрисовке, а считалась только внутри разреза —
        // свежий экземпляр абзаца оставался с нулём.
        if self.hyphen_w.get() == px(0.)
            && !self.hyphen.is_empty()
            && self.text.contains('\u{00ad}')
        {
            let mark = self.hyphen.clone();
            self.hyphen_w.set(self.suffix_width(&mark, 0, window));
        }
        // Подбор кегля мутирует абзац между вызовами — ключ это видит
        // (font_size в ключе замера).
        let key = self.split_key(limit);
        if let Some(hit) = SPLITS.with(|c| c.borrow().get(&key).cloned()) {
            return (*hit).clone();
        }
        let lines = self.split_uncached(limit, window);
        SPLITS.with(|c| {
            let mut m = c.borrow_mut();
            // Прямолинейный сброс при переполнении: страница с тысячами
            // абзацев дороже промахов одного сброса.
            if m.len() >= 2048 {
                m.clear();
            }
            m.insert(key, std::rc::Rc::new(lines.clone()));
        });
        lines
    }

    fn split_uncached(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        let segs = self.measure(window);
        let mut lines = self.lay(limit, &segs);
        // Обрезка строк контейнером с `text-overflow: ellipsis`: не влезшая
        // строка усекается с многоточием (css-overflow-3 §text-overflow).
        if self.text_overflow
            && let Some(limit) = limit
        {
            for line in lines.iter_mut() {
                if line.width > limit + px(0.5) && !line.ellipsis {
                    self.ellipsize(line, limit, &segs, window);
                }
            }
        }
        let cut = self
            .clamp
            .filter(|n| *n > 0 && lines.len() > *n)
            .zip(limit)
            .filter(|_| self.wrap.balance);
        let Some((max, limit)) = cut else {
            return self.clamp_lines(self.balanced(lines, limit, &segs), limit, window);
        };
        // Видимый текст — тот, что уместился в обрезанные строки. Его и
        // раскладываем заново, ища самую узкую колонку, в которой он всё ещё
        // помещается в те же строки.
        let end = self
            .clamp_lines(lines, Some(limit), window)
            .last()
            .map(|l| l.range.end)
            .unwrap_or(0);
        // ЗАМЕРЕНО И ОТКАЧЕНО: резервировать при подборе место под
        // МНОГОТОЧИЕ (условие `l.width + ell <= middle`). Срез из 27 пар
        // семей balance/clamp/text-wrap: 0 и 0 — колонка, к которой сходится
        // двоичный поиск, от этого условия не меняется.
        // `text-wrap-balance-line-clamp-*` держит не подбор ширины.
        // Место под МНОГОТОЧИЕ входит в колонку: на оборванной строке за
        // текстом рисуется знак обрыва, и колонка, в которую он не влезает,
        // подбором не годится (css-text-4 §5: обрыв — часть последней
        // строки). Прошлый заход мерил ширину строки КАК ЕСТЬ; здесь хвост
        // сперва обрезается, как это делает сам обрыв (`ellipsize`).
        let ell = self.suffix_width(self.marker_str(), 0, window);
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            let probe = self.lay(Some(middle), &segs);
            let fits = probe.get(max - 1).is_some_and(|l: &Line| {
                if l.range.end < end {
                    return false;
                }
                let cut = l.range.start + trim_hanging(&self.text[l.range.clone()]);
                self.span(&segs, l.range.start, cut) + ell <= middle
            });
            if fits {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.clamp_lines(self.lay(Some(wide), &segs), Some(limit), window)
    }

    /// `line-clamp`: строк остаётся не больше заданного числа, а на последней
    /// появляется многоточие. Место под него отбирается у текста — иначе
    /// строка вылезала бы за коробку (`text-wrap-balance-line-clamp-003`).
    fn clamp_lines(
        &self,
        mut lines: Vec<Line>,
        limit: Option<Pixels>,
        window: &mut Window,
    ) -> Vec<Line> {
        let Some(max) = self.clamp.filter(|n| *n > 0) else {
            return lines;
        };
        // Обрывать нечего — знака нет. Исключение — авто-режим, где точка
        // среза бывает МЕЖДУ блоками: абзац видим целиком, а знак ему всё
        // равно положен, потому что за ним обрывается содержимое
        // контейнера (`line-clamp-auto-024`: срез между вторым и третьим
        // блоком, а «…» — на «Line 6»).
        if lines.len() <= max && !self.clamp_force {
            return lines;
        }
        lines.truncate(max);
        let Some(last) = lines.pop() else {
            return lines;
        };
        let segs = self.measure(window);
        let ell = self.suffix_width(self.marker_str(), last.range.start, window);
        let head = last.range.start;
        let mut end = head + trim_hanging(&self.text[last.range.clone()]);
        // Место под многоточие отбирается ЦЕЛЫМИ кусками: строка обрывается по
        // точке переноса, а не посреди слова. Слово, которое с многоточием уже
        // не влезает, уходит со строки целиком — как в браузере.
        if let Some(room) = limit.map(|w| w - ell) {
            if self.span(&segs, head, end) > room {
                end = self
                    .opportunities()
                    .iter()
                    .map(|s| s.at)
                    .filter(|at| *at > head && *at <= end)
                    .map(|at| head + trim_hanging(&self.text[head..at]))
                    .filter(|at| self.span(&segs, head, *at) <= room)
                    .max()
                    .unwrap_or(head);
            }
        }
        let width = self.span(&segs, head, end) + ell;
        lines.push(Line {
            range: head..end,
            width,
            ellipsis: true,
            hyphen: false,
            indent: last.indent,
        });
        lines
    }

    /// `line-clamp`: сколько строк оставить.
    pub fn line_clamp(mut self, lines: Option<usize>) -> Self {
        self.clamp = lines;
        self
    }

    /// Ставить знак обрыва и тогда, когда абзац влез целиком: точка среза
    /// стоит сразу за ним (авто-режим, css-overflow-4 §5.3).
    pub fn clamp_marked(mut self, on: bool) -> Self {
        self.clamp_force = on;
        self
    }

    /// `text-overflow: ellipsis` контейнера: строка, не влезшая в колонку
    /// (nowrap/pre — переносов нет), усекается и получает многоточие.
    pub fn text_ellipsis(mut self, on: bool) -> Self {
        self.text_overflow = on;
        self
    }

    /// Маркер обрезки: строка из `text-overflow: <string>`, шрифт И КЕГЛЬ
    /// блока (css-overflow-4 §5 — маркер оформлен как блок).
    pub fn overflow_marker(
        mut self,
        mark: Option<String>,
        font: Option<gpui::Font>,
        size: Option<Pixels>,
    ) -> Self {
        self.overflow_marker = mark;
        self.marker_font = font;
        self.marker_size = size;
        self
    }

    /// Цвет знака обрыва — цвет блока.
    pub fn marker_color(mut self, color: Option<Hsla>) -> Self {
        self.marker_color = color;
        self
    }

    /// Усечь строку под многоточие: место отбирается целыми кусками по
    /// точкам переноса — как у `line-clamp` (общая механика).
    fn ellipsize(&self, line: &mut Line, limit: Pixels, segs: &[Seg], window: &mut Window) {
        let ell = self.suffix_width(self.marker_str(), line.range.start, window);
        let head = line.range.start;
        let mut end = head + trim_hanging(&self.text[line.range.clone()]);
        let room = limit - ell;
        if self.wrap.rtl {
            // Письмо справа налево: строка прижата вправо, контейнер режет
            // ЛЕВЫЙ край — усечение с ЛОГИЧЕСКОГО НАЧАЛА, многоточие там же.
            let mut start = head;
            if self.span(segs, head, end) > room {
                start = self.text[head..end]
                    .char_indices()
                    .map(|(i, _)| head + i)
                    .filter(|at| *at > head && self.span(segs, *at, end) <= room)
                    .min()
                    .unwrap_or(end);
            }
            line.width = self.span(segs, start, end) + ell;
            line.range = start..end;
            line.ellipsis = true;
            return;
        }
        if self.span(segs, head, end) > room {
            let by_break = self
                .opportunities()
                .iter()
                .map(|s| s.at)
                .filter(|at| *at > head && *at <= end)
                .map(|at| head + trim_hanging(&self.text[head..at]))
                .filter(|at| self.span(segs, head, *at) <= room)
                .max();
            // Непереносимое слово режется ПО ЗНАКАМ: обрезка контейнером
            // не ждёт точки переноса (в отличие от line-clamp).
            end = by_break.unwrap_or_else(|| {
                self.text[head..end]
                    .char_indices()
                    .map(|(i, _)| head + i)
                    .filter(|at| *at > head && self.span(segs, head, *at) <= room)
                    .max()
                    .unwrap_or(head)
            });
        }
        line.width = self.span(segs, head, end) + ell;
        line.range = head..end;
        line.ellipsis = true;
    }

    /// Чем показывать перенос слова.
    pub fn hyphen_char(mut self, mark: Option<String>) -> Self {
        if let Some(mark) = mark {
            self.hyphen = SharedString::from(mark);
        }
        self
    }

    /// Шаг позиций табуляции.
    pub fn tab_stops(mut self, step: tabs::TabStops) -> Self {
        self.tab_stop = step;
        self
    }

    /// Куски вне потока: место в тексте → элемент.
    pub fn overlays(mut self, overlays: Vec<(usize, AnyElement, crate::inline::OverlayAt)>) -> Self {
        self.overlays = overlays;
        self
    }

    /// Атомы в строке: место в тексте (байт распорки) → элемент и его
    /// `vertical-align`. Каждый заворачивается в ряд `align-items: baseline`
    /// со щупом базовой линии (см. `BaselineProbe`): обёртка обтягивает
    /// коробку полей атома, щуп отдаёт её базовую линию.
    pub fn atoms(mut self, atoms: Vec<(usize, AnyElement, AtomAlign, RubyExtents)>) -> Self {
        use gpui::{ParentElement, Styled};
        // Распорка атома — не распорка полей: из текста для переноса её не
        // вынимают, а читают знаком-заместителем (см. `linebreaks`).
        self.spacers
            .retain(|s| !atoms.iter().any(|(at, _, _, _)| at == s));
        self.atoms = atoms
            .into_iter()
            .map(|(at, el, align, extents)| {
                let probe = std::rc::Rc::new(std::cell::Cell::new(None));
                let root = std::rc::Rc::new(std::cell::Cell::new(None));
                let el = LayoutTap {
                    child: gpui::div()
                        .flex()
                        .items_baseline()
                        .child(BaselineProbe {
                            slot: probe.clone(),
                        })
                        .child(el)
                        .into_any_element(),
                    slot: root.clone(),
                }
                .into_any_element();
                AtomSlot {
                    at,
                    el,
                    align,
                    probe,
                    root,
                    extents,
                    hidden: false,
                }
            })
            .collect();
        self
    }

    /// Верх атома от базовой линии строки (ось вниз) — для всех выравниваний,
    /// кроме `top`/`bottom`: те зависят от готовой строки (§10.8.1 «aligned
    /// subtree» решается после остальных).
    fn atom_top(&self, b: &AtomBox) -> f32 {
        let (asc, desc, xh) = self.strut;
        match b.align {
            AtomAlign::Shift(v) => -b.base - v,
            AtomAlign::Middle => -xh / 2.0 - b.h / 2.0,
            AtomAlign::TextTop => -asc,
            AtomAlign::TextBottom => desc - b.h,
            AtomAlign::Top | AtomAlign::Bottom => -b.base,
        }
    }

    /// Протяжённость строки над и под её базовой линией по строчным коробкам
    /// (CSS 2.1 §10.8.1): у каждой коробки `A = (L − (a + d)) / 2 + a` над
    /// базовой и `L − A` под ней, где `L` — её `line-height`, `a`/`d` —
    /// подъём и спуск её шрифта; струт блока входит всегда. Сдвиг
    /// `vertical-align` (`shift_spans`, ось вниз) двигает коробку целиком.
    /// Blink: `inline_box_state.cc` `ComputeTextMetrics` + `line_box_fragment_
    /// builder` — та же сумма наибольших подъёма и спуска.
    fn line_extents(&self, range: &std::ops::Range<usize>) -> (f32, f32) {
        let lh = f32::from(self.line_height);
        let (sa, sd) = self.strut_box;
        let a_s = (lh - (sa + sd)) / 2.0 + sa;
        let (mut top, mut bot) = (a_s, lh - a_s);
        let mut at = 0usize;
        for (run, &(ra, rd)) in self.runs.iter().zip(&self.run_metrics) {
            let (s, e) = (at, at + run.len);
            at = e;
            if run.len == 0 || e <= range.start || s >= range.end {
                continue;
            }
            // Кусок у края строки равняется по готовой строке (`edge_spans`).
            if self
                .edge_spans
                .iter()
                .any(|(r, _, _)| r.start <= s && e <= r.end)
            {
                continue;
            }
            let own = self
                .box_spans
                .iter()
                .find(|(r, _)| r.contains(&s))
                .map_or(lh, |(_, v)| *v);
            let a_r = (own - (ra + rd)) / 2.0 + ra;
            let dy = self
                .shift_spans
                .iter()
                .find(|(r, _)| r.contains(&s))
                .map_or(0.0, |(_, v)| f32::from(*v));
            top = top.max(a_r - dy);
            bot = bot.max(own - a_r + dy);
        }
        (top, bot)
    }

    /// От верха струта до его базовой линии: полулидинг плюс подъём (§10.8.1).
    fn strut_base(&self) -> f32 {
        let (asc, desc, _) = self.strut;
        (f32::from(self.line_height) - (asc + desc)) / 2.0 + asc
    }

    /// Подъём и спуск основного шрифта каждого прогона.
    fn measure_runs(&self, window: &mut Window) -> Vec<(f32, f32)> {
        self.runs
            .iter()
            .map(|run| {
                let id = window.text_system().resolve_font(&run.font);
                let size = run.font_size.unwrap_or(self.font_size);
                let ts = window.text_system();
                (
                    f32::from(ts.ascent(id, size)),
                    f32::from(ts.descent(id, size)).abs(),
                )
            })
            .collect()
    }

    /// Базовая линия отрезка от верха его строки: наибольшие подъём и спуск
    /// прогонов отрезка, полулидинг от высоты строки — ровно как кладёт глифы
    /// сплошной набор (`padding_top + ascent` в `vendor/gpui/.../line.rs`).
    fn base_of(&self, range: &std::ops::Range<usize>) -> Option<f32> {
        if self.run_metrics.len() != self.runs.len() {
            return None;
        }
        let mut at = 0usize;
        let mut best: Option<(f32, f32)> = None;
        for (run, &(a, d)) in self.runs.iter().zip(&self.run_metrics) {
            let (s, e) = (at, at + run.len);
            at = e;
            if run.len == 0 || e <= range.start || s >= range.end {
                continue;
            }
            // Прогон у края строки в базовую линию не входит: его коробка
            // равняется по краю, а не по базовой (§10.8.1). Отрезок самого
            // такого куска своей базовой не меряет — там прогон считается.
            let edge = self.edge_spans.iter().any(|(r, _, _)| {
                r.start <= s && e <= r.end && !(r.start <= range.start && range.end <= r.end)
            });
            if edge {
                continue;
            }
            best = Some(best.map_or((a, d), |(ba, bd)| (ba.max(a), bd.max(d))));
        }
        best.map(|(a, d)| (f32::from(self.line_height) - (a + d)) / 2.0 + a)
    }

    /// Базовая линия строки: по её прогонам, без них — по струту.
    fn line_base(&self, range: &std::ops::Range<usize>) -> f32 {
        self.base_of(range).unwrap_or_else(|| self.strut_base())
    }

    /// Раскладка атомов ДО замера абзаца: перенос и высота строк зависят от
    /// их размеров, а внутри замера раскладывать нельзя (движок раскладки
    /// занят). Атом меряется по содержимому — в строку допускаются только
    /// атомы, чей размер от ширины строки не зависит (решает `render.rs`).
    fn lay_atoms(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(run) = self.runs.first() {
            let id = window.text_system().resolve_font(&run.font);
            let size = run.font_size.unwrap_or(self.font_size);
            let ts = window.text_system();
            self.strut = (
                f32::from(ts.ascent(id, size)),
                f32::from(ts.descent(id, size)).abs(),
                f32::from(ts.x_height(id, size)),
            );
        }
        self.run_metrics = self.measure_runs(window);
        self.atom_boxes.clear();
        for slot in self.atoms.iter_mut() {
            let rounded = slot.el.layout_as_root(
                size(
                    gpui::AvailableSpace::MaxContent,
                    gpui::AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            // Размер и базовая линия — ТОЧНЫЕ, без округления к точке
            // устройства (см. `LayoutTap`); щуп стоит прямо в обёртке, и его
            // смещение от неё и есть базовая линия.
            let s = slot
                .root
                .get()
                .map(|id| window.layout_exact(id).1)
                .unwrap_or(rounded);
            let base = slot
                .probe
                .get()
                .map(|id| f32::from(window.layout_exact(id).0.y))
                .unwrap_or(f32::from(s.height));
            // Базовая на самом ВЕРХУ коробки — признак того, что раскладка
            // базовой линии не нашла (таблица с пустой ячейкой отдаёт ноль).
            // По CSS 2.1 §17.5.3 и §10.8.1 тогда это низ коробки: «If there is
            // no such line box or table-row, the baseline is the bottom of
            // content edge of the cell box» (`min-height-applies-to-014`).
            let base = if base <= 0.0 && s.height > px(0.) {
                f32::from(s.height)
            } else {
                base
            };
            let w = f32::from(s.width);
            // Уровни одной стороны стоят стопкой в каждой колонке; выход за
            // коробку — по самой высокой стопке.
            let (mut over, mut under) = (0.0f32, 0.0f32);
            for (below, inset, id) in &slot.extents {
                let Some(id) = id.get() else { continue };
                let h = f32::from(window.layout_exact(id).1.height) - inset;
                if *below {
                    under = under.max(h);
                } else {
                    over = over.max(h);
                }
            }
            self.atom_boxes.push(AtomBox {
                at: slot.at,
                h: f32::from(s.height),
                base,
                align: slot.align,
                over,
                under,
            });
            // Продвижение распорки — ширина атома. Идёт ПЕРВЫМ: поиск
            // диапазона берёт первое попадание.
            let len = self.text[slot.at..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
            self.letter_spans.insert(0, (slot.at..slot.at + len, px(w)));
        }
    }

    /// Поставить атомы на места их строк: x — от продвижения до распорки
    /// плюс прижим строки (тот же, что у отрисовки), y — от базовой линии
    /// строки по `vertical-align`.
    /// Трекинг, который добавлен ПОСЛЕДНЕМУ знаку отрезка.
    ///
    /// По css-text-3 §8.2 межбуквенный интервал в конце строки не действует:
    /// он свисает за край и в ширину строки не входит. Пока входил, коробка
    /// шириной ровно в текст рвала последнее слово (`letter-spacing-200`).
    fn tail_spacing(&self, end: usize) -> Pixels {
        if end == 0 {
            return px(0.);
        }
        // Знак-распорка несёт ПОЛЕ строчной коробки, а не трекинг: вычитать
        // его на конце строки нельзя — иначе коробка теряет своё правое поле
        // и выходит уже на целый em (`word-space-transform-010`).
        if self.text[..end].chars().next_back() == Some('\u{feff}') {
            return px(0.);
        }
        let at = self.text[..end]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.letter_spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, v)| *v)
            .unwrap_or(self.letter_spacing)
    }

    /// Выключка строки `i`: последняя строка и строка перед жёстким разрывом
    /// идут своей выключкой (`text-align-last`), `plaintext` решает сторону
    /// по абзацу между разрывами. Общая для отрисовки и для мест атомов.
    fn line_align(&self, i: usize, line: &Line) -> Align {
        let count = self.lines.len();
        let body = self.text[line.range.clone()].trim_end_matches('\n');
        let last_line = i + 1 == count || body.len() < line.range.len();
        // Строка с СОХРАНЁННОЙ табуляцией не растягивается (позиции
        // табуляции обязаны совпасть с нерастянутой строкой), но выключку
        // ПОСЛЕДНЕЙ строки (`text-align-last`) она не получает: к
        // табуляции та отношения не имеет.
        let no_stretch = last_line;
        // При `plaintext` сторона письма своя у каждого АБЗАЦА между
        // жёсткими разрывами (не у строки: мягкий перенос сторону не
        // меняет). От неё же зависят `start` и `end`.
        let own_align =
            match self.plaintext {
                Some(logical) => {
                    let start = self.text[..line.range.start]
                        .rfind('\n')
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    let end = self.text[start..]
                        .find('\n')
                        .map(|i| start + i)
                        .unwrap_or(self.text.len());
                    // При `unicode-bidi: plaintext` сторона КАЖДОГО абзаца
                    // берётся по первому сильному знаку (css-writing-modes-4
                    // §2.2 -> UAX#9 P2/P3), а не у элемента. Порядок глифов это
                    // уже учитывал (`BidiInfo::new(text, None)`), выключка —
                    // нет. Нейтральный абзац сильного знака не имеет и остаётся
                    // на стороне элемента.
                    align_of_value(logical.physical(
                        first_strong_rtl(&self.text[start..end]).unwrap_or(self.wrap.rtl),
                    ))
                }
                None => self.align,
            };
        // Нерастянутая выключка: `justify` прижимает строку к НАЧАЛУ, а
        // начало у письма справа налево — правый край, не левый.
        let flat = |a: Align| match a {
            Align::Justify if self.wrap.rtl => Align::Right,
            Align::Justify => Align::Left,
            other => other,
        };
        let align = if last_line {
            self.align_last.unwrap_or(flat(own_align))
        } else if no_stretch {
            flat(own_align)
        } else {
            own_align
        };
        self.ruby_line_align(align, &line.range)
    }

    /// Где в коробке стоит байт текста: левый верхний угол его знака.
    fn point_of(&self, segs: &[Seg], at: usize, bounds: Bounds<Pixels>) -> Point<Pixels> {
        let row = self
            .lines
            .iter()
            .position(|l| at < l.range.end)
            .unwrap_or(self.lines.len().saturating_sub(1));
        let Some(line) = self.lines.get(row) else {
            return bounds.origin;
        };
        let from = self.x_at(segs, line.range.start, Edge::Start);
        let x = self.x_at(segs, at.max(line.range.start), Edge::Start) - from;
        // Стартовое смещение строки — как у отрисовки: отступ первой строки,
        // свисающий открывающий знак, левый вырез обтекания. Без него точка
        // жила от голого края коробки, и статическая позиция абсолюта в
        // строке с `text-indent` промахивалась ровно на отступ
        // (htb-ltr-*: регресс 08-12, зелёные квадраты не закрывали красное).
        let hang = self.hang_first(line.range.start);
        let shift = self.span(segs, line.range.start, line.range.start + hang);
        let lead =
            if self.wrap.rtl { px(0.) } else { line.indent } - shift + px(self.flow_cut(row).0);
        // Повёрнутый абзац при `direction: rtl`: место считается ВИЗУАЛЬНО
        // (`visual_x_rtl`) — строка прижата к правому краю до-поворотной
        // коробки и переставлена разбором UAX#9, а логическое продвижение от
        // левого края верно только для одного rtl-прогона.
        if self.wrap.rtl && crate::interact::in_rotated_frame() {
            let free_raw =
                bounds.size.width - line.width - line.indent - px(self.flow_cut(row).1);
            let left = bounds.origin.x
                + line_offset(self.line_align(row, line), true, free_raw)
                - shift
                + px(self.flow_cut(row).0);
            let visual = if self.lines_reversed {
                self.lines.len().saturating_sub(1).saturating_sub(row)
            } else {
                row
            };
            return point(
                left + self.visual_x_rtl(segs, at, line),
                bounds.origin.y + self.line_height * visual as f32,
            );
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО (04.09): прибавлять сюда долю ВЫКЛЮЧКИ
        // (`text-align: center|right`) тем же счётом, что и отрисовка
        // (`free/2` и `free`). Срез из 633 пар статической позиции и
        // абсолютов: 382 -> 359, приобретено 0, потеряно 23 — вся семья
        // `abs-pos-non-replaced-v{lr,rl}-1xx` (0.00 -> 2.67) и
        // `abspos-width-change-inline-container-001`. В повёрнутом абзаце
        // `bounds.size.width` — не та ось, и остаток строки считается не от
        // той стороны; возвращать вместе с осевым остатком.
        // Строки рисуются снизу вверх (`lines_reversed` — это `vertical-lr`),
        // и НОМЕР строки в списке тогда зеркален её месту на экране. Щуп
        // статической позиции брал номер как есть и садился на зеркальную
        // строку — оттого вся семья `abs-pos-non-replaced-vlr-*` промахивалась
        // ровно на отражение, а `-vrl-*` (там порядок прямой) была цела.
        let visual = if self.lines_reversed {
            self.lines.len().saturating_sub(1).saturating_sub(row)
        } else {
            row
        };
        point(
            bounds.origin.x + lead + x,
            bounds.origin.y + self.line_height * visual as f32,
        )
    }

    /// Визуальное продвижение места `at` от ЛЕВОГО края rtl-строки.
    ///
    /// На место куска ставится нейтральный U+FFFC (так UAX#9 видит
    /// замещаемый объект), строка разбирается с базой rtl, и прогоны идут в
    /// ВИЗУАЛЬНОМ порядке (L2), как у отрисовки (`paint_line`): слева
    /// складываются ширины прогонов до прогона метки, внутри него — знаки
    /// до метки (ltr-прогон) или после неё (rtl-прогон). Пример
    /// `abs-pos-non-replaced-vrl-008`: строка «34» + абсолют — метка уровня 1
    /// после числа уровня 2 встаёт ЛЕВЕЕ числа, и коробка висит от левого
    /// края строки, а не от правого.
    fn visual_x_rtl(&self, segs: &[Seg], at: usize, line: &Line) -> Pixels {
        let start = line.range.start;
        let end = start + trim_hanging(&self.text[line.range.clone()]);
        let at = at.clamp(start, end);
        const MARK: usize = 3; // U+FFFC в UTF-8
        let mut probe = String::with_capacity(self.text.len() + MARK);
        probe.push_str(&self.text[..at]);
        probe.push('\u{fffc}');
        probe.push_str(&self.text[at..]);
        let info = unicode_bidi::BidiInfo::new(&probe, Some(unicode_bidi::Level::rtl()));
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= start && start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return px(0.);
        };
        let (levels, runs) = info.visual_runs(para, start..end + MARK);
        // Отрезок метки-строки обратно в отрезок исходного текста.
        let orig = |p: usize| if p <= at { p } else { p - MARK };
        let width = |a: usize, b: usize| self.span(segs, orig(a), orig(b));
        let mut x = px(0.);
        for run in runs {
            let mark_in = run.start <= at && at < run.end;
            if !mark_in {
                x += width(run.start, run.end);
                continue;
            }
            let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
            x += if rtl {
                width(at + MARK, run.end)
            } else {
                width(run.start, at)
            };
            break;
        }
        x
    }

    /// Уровень bidi у места куска вне потока — справа налево ли? Сам кусок
    /// в тексте знака не имеет, поэтому на его место ставится нейтральный
    /// U+FFFC (так UAX#9 видит замещаемый объект): его уровень решают
    /// соседи по правилам N1/N2.
    fn rtl_level_at(&self, at: usize) -> bool {
        let at = at.min(self.text.len());
        let mut probe = String::with_capacity(self.text.len() + 3);
        probe.push_str(&self.text[..at]);
        probe.push('\u{fffc}');
        probe.push_str(&self.text[at..]);
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        let forced = if self.plaintext.is_some() { None } else { Some(base) };
        let info = unicode_bidi::BidiInfo::new(&probe, forced);
        info.levels.get(at).map_or(self.wrap.rtl, |l| l.is_rtl())
    }

    /// Статическая позиция БЛОЧНОГО куска вне потока: строчное начало —
    /// край содержимого (без `text-indent`: отступ — свойство первой
    /// СТРОКИ, а гипотетическая коробка — блок), блочное — начало строки,
    /// следующей за той, где кусок стоит в тексте (CSS 2.1 §10.6.4 «if
    /// position had been static»: блок в строчном содержимом рвёт строку и
    /// встаёт после неё). Кусок в самом начале строки ничего перед собой не
    /// имеет — строка рвётся ДО него, и место — верх этой же строки.
    /// Порядок строк на экране при `lines_reversed` зеркален (см. `point_of`).
    fn next_line_point(&self, at: usize, bounds: Bounds<Pixels>) -> Point<Pixels> {
        let count = self.lines.len();
        let row = self
            .lines
            .iter()
            .position(|l| at < l.range.end)
            .unwrap_or(count.saturating_sub(1));
        // Есть ли перед куском в его строке настоящее содержимое (служебные
        // распорки `SPACER`/`ZWSP` места не занимают).
        let after = self.lines.get(row).is_some_and(|l| {
            let end = at.clamp(l.range.start, l.range.end);
            self.text
                .get(l.range.start..end)
                .is_some_and(|t| t.chars().any(|c| !matches!(c, '\u{feff}' | '\u{200b}')))
        });
        let visual = if self.lines_reversed {
            count.saturating_sub(1).saturating_sub(row) as f32
        } else {
            row as f32
        };
        // Следующая строка: при обратном порядке она ВЫШЕ на экране.
        let step = match (after, self.lines_reversed) {
            (false, _) => 0.0,
            (true, false) => 1.0,
            (true, true) => -1.0,
        };
        point(bounds.origin.x, bounds.origin.y + self.line_height * (visual + step))
    }

    /// `text-fit`: подбирать ли кегль под ширину коробки.
    pub fn text_fit(mut self, fit: Option<crate::computed::TextFit>) -> Self {
        self.fit = fit;
        self
    }

    /// Какие части абзаца подбор кегля вправе масштабировать.
    pub fn fit_parts(mut self, spacing_scalable: bool, line_height_fixed: bool) -> Self {
        self.fit_spacing_scalable = spacing_scalable;
        self.fit_line_height_fixed = line_height_fixed;
        self
    }

    /// Подобрать кегль так, чтобы строки заполнили коробку (css-text-5).
    ///
    /// Считается по САМОЙ ШИРОКОЙ строке: увеличивать до тех пор, пока она не
    /// упрётся в край. Множитель идёт на всё, что задаёт размер набора, —
    /// кегль, интерлиньяж и разрядки, иначе строка растёт непропорционально.
    fn apply_fit(&mut self, limit: Pixels, window: &mut Window) {
        let Some(f) = self.fit.filter(|f| f.grow || f.shrink) else {
            return;
        };
        let lines = self.split(Some(limit), window);
        let Some(widest_line) = lines.iter().max_by(|a, b| {
            a.width
                .partial_cmp(&b.width)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            return;
        };
        let widest = widest_line.width;
        if widest <= px(0.) || limit <= px(0.) {
            return;
        }
        // Немасштабируемые части строки (css-text-5 §text-fit: интервалы в
        // точках и `em`) в подборе не участвуют: множитель считается как
        // (A + B) / A, где A — масштабируемая ширина, B — остаток места.
        // Иначе `letter-spacing: 10px` рос вместе с глифами, сумма сходилась,
        // а глифы выходили не те (`text-fit/spacing`).
        let fixed = if self.fit_spacing_scalable {
            0.0
        } else {
            let text = &self.text[widest_line.range.clone()];
            let chars = text.chars().count().max(1) as f32;
            let spaces = text.chars().filter(|c| c.is_whitespace()).count() as f32;
            f32::from(self.letter_spacing) * (chars - 1.0) + f32::from(self.word_spacing) * spaces
        };
        let scalable = f32::from(widest) - fixed;
        if scalable <= 0.0 {
            return;
        }
        let mut k = (f32::from(limit) - fixed) / scalable;
        // Процент — ЗАЖИМ множителя (css-text-5): при `grow` и ≥ 100% —
        // максимум, при `shrink` и ≤ 100% — минимум; иначе предела нет.
        // Раньше он множился как доля заполнения, и `shrink 75%` давал кегль
        // вдвое меньше нужного (`shrink-per-line-all`).
        if let Some(t) = f.target {
            if f.grow && t >= 1.0 {
                k = k.min(t);
            }
            if f.shrink && t <= 1.0 {
                k = k.max(t);
            }
        }
        if !k.is_finite() || (k > 1.0 && !f.grow) || (k < 1.0 && !f.shrink) {
            return;
        }
        if (k - 1.0).abs() < 0.001 {
            return;
        }
        remember_fit(self.measure_key(), k);
        self.scale_by(k);
    }

    /// Множитель, найденный ЗАМЕРОМ для этого же абзаца.
    ///
    /// Подбирать кегль имеет смысл только под заданный размер строки, а
    /// известен он лишь замеру: отрисовке коробка достаётся уже посчитанной, и
    /// по ней подбор пошёл бы по кругу (`text-fit/writing-mode`: кегль
    /// вырастал в размер окна).
    fn apply_measured_fit(&mut self) {
        if self.fit.is_none() {
            return;
        }
        let key = self.measure_key();
        if let Some(k) = FITTED.with(|c| {
            c.borrow()
                .iter()
                .find(|(hit, _)| *hit == key)
                .map(|(_, k)| *k)
        }) {
            self.scale_by(k);
        }
        self.fit = None;
    }

    /// Помножить всё, что задаёт размер набора.
    fn scale_by(&mut self, k: f32) {
        self.font_size = px(f32::from(self.font_size) * k);
        // Заданная длиной `line-height` от подбора не зависит (css-text-5:
        // «line-height: 1.5em … are not affected by this scaling»); растёт
        // только `normal` и число — они считаются от использованного кегля.
        if !self.fit_line_height_fixed {
            self.line_height = px(f32::from(self.line_height) * k);
            for (_, lh) in &mut self.lh_spans {
                *lh = px(f32::from(*lh) * k);
            }
        }
        if self.fit_spacing_scalable {
            self.letter_spacing = px(f32::from(self.letter_spacing) * k);
            self.word_spacing = px(f32::from(self.word_spacing) * k);
        }
        for run in &mut self.runs {
            if let Some(size) = run.font_size {
                run.font_size = Some(px(f32::from(size) * k));
            }
        }
    }

    /// Ширина многоточия в наборе того куска, где оборвана строка.
    /// Маркер обрезки: свой из `text-overflow: <string>` либо многоточие.
    fn marker_str(&self) -> &str {
        self.overflow_marker.as_deref().unwrap_or(ELLIPSIS)
    }

    fn suffix_width(&self, mark: &str, at: usize, window: &mut Window) -> Pixels {
        let mut runs = slice_runs(&self.runs, &(at..at + 1));
        let Some(run) = runs.first_mut() else {
            return px(0.);
        };
        run.len = mark.len();
        self.style_marker_run(mark, run);
        let piece = vec![run.clone()];
        window
            .text_system()
            .shape_line_spaced(
                SharedString::from(mark.to_string()),
                self.font_size,
                &piece,
                None,
                self.letter_spacing,
            )
            .width
    }

    /// `text-wrap: balance` — те же строки, но одной длины.
    fn balanced(&self, lines: Vec<Line>, limit: Option<Pixels>, segs: &[Seg]) -> Vec<Line> {
        let Some(limit) = limit else { return lines };
        if !self.wrap.balance || lines.len() < 2 {
            return lines;
        }
        // Группы строк, разделённые ЖЁСТКИМ разрывом, выравниваются по
        // отдельности (css-text-4 §7.1): у каждой своя ширина, одной на весь
        // абзац не хватает (`text-wrap-balance-004`).
        let mut out: Vec<Line> = Vec::new();
        let mut i = 0usize;
        while i < lines.len() {
            let last = lines[i..]
                .iter()
                .position(|l| self.text[l.range.clone()].ends_with('\n'))
                .map(|k| i + k)
                .unwrap_or(lines.len() - 1);
            let part = lines[i].range.start..lines[last].range.end;
            out.extend(self.balanced_part(part, last + 1 - i, limit, segs));
            i = last + 1;
        }
        out
    }

    /// Выравнивание длин ОДНОЙ группы строк: поиск самой узкой колонки, в
    /// которой строк не прибавилось. Тогда последняя строка перестаёт быть
    /// коротким огрызком.
    fn balanced_part(
        &self,
        part: std::ops::Range<usize>,
        target: usize,
        limit: Pixels,
        segs: &[Seg],
    ) -> Vec<Line> {
        if target < 2 {
            return self.lay_in(part, Some(limit), segs);
        }
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            if self.lay_in(part.clone(), Some(middle), segs).len() <= target {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.lay_in(part, Some(wide), segs)
    }

    /// Набор строк под заданную ширину — без выравнивания их длин.
    fn lay(&self, limit: Option<Pixels>, segs: &[Seg]) -> Vec<Line> {
        self.lay_in(0..self.text.len(), limit, segs)
    }

    /// То же для ЧАСТИ текста: выравнивание длин идёт по группам между
    /// жёсткими разрывами, и каждая группа набирается своей ширины.
    fn lay_in(
        &self,
        part: std::ops::Range<usize>,
        limit: Option<Pixels>,
        segs: &[Seg],
    ) -> Vec<Line> {
        let x = |i: usize| -> Pixels { self.x_at(segs, i, Edge::End) };
        let mut out: Vec<Line> = Vec::new();
        // Начало строки: схлопываемые пробелы после переноса не рисуются и в
        // ширину не входят. При сохранённых пробелах (`pre*`) они значимы.
        // Правило берётся В ЭТОМ МЕСТЕ, а не у абзаца целиком: `white-space`
        // на вложенном `<span>`/`display: inline` действует на свои знаки, и
        // абзац об этом не знает. Пока смотрели правило абзаца, сохранённые
        // пробелы вложенного куска исчезали с начала перенесённой строки
        // (`ws-break-spaces-applies-to-001`).
        let bol = |at: usize| -> usize {
            if self.wrap_at(at).keep_spaces {
                at
            } else {
                at + skip_leading(&self.text[at..])
            }
        };
        let mut start = bol(part.start);
        // Отступ первой строки (`text-indent`) — свойство СТРОКИ, а не абзаца:
        // его получает первая строка блока, при `each-line` — первая после
        // каждого жёсткого разрыва, при `hanging` — все остальные. Поэтому
        // здесь ведётся, начинает ли строка кусок и первый ли это кусок блока:
        // группы между жёсткими разрывами набираются и по отдельности
        // (выравнивание длин), и подряд в одном проходе.
        let mut head_of_part = true;
        let mut first_part = part.start == 0;
        let mut last_fit: Option<usize> = None;
        let mut opportunities: Vec<Stop> = self
            .opportunities()
            .into_iter()
            .filter(|s| s.at > part.start && s.at <= part.end)
            .collect();
        // Конец текста — тоже точка проверки: без него хвост последней строки
        // никто не мерил и она оставалась во всю длину, сколько бы ни
        // переполняла коробку.
        if opportunities.last().is_none_or(|s| s.at < part.end) {
            opportunities.push(Stop {
                at: part.end,
                mandatory: false,
            });
        }
        let mut i = 0usize;
        while i < opportunities.len() {
            let Stop { at, mandatory } = opportunities[i];
            if at <= start {
                i += 1;
                continue;
            }
            // Отступ отбирает место у СВОЕЙ строки: на неё остаётся уже
            // меньшая ширина, а отрицательный отступ, наоборот, добавляет.
            let ind = self.indent_of(head_of_part, first_part, limit);
            // Вырез обтекания сужает СВОЮ строку: левый входит в отступ
            // строки, правый просто отбирает ширину (css-shapes-1 §2).
            let (fl, fr) = self.flow_cut(out.len());
            let ind = ind + px(fl);
            let limit = limit.map(|w| w - ind - px(fr));
            // Хвостовые пробелы висят за краем: в ширину строки они не входят.
            // Хвостовые пробелы висят за краем СТРОКИ — то есть когда край
            // вообще есть. При замере по максимальному содержимому предела
            // нет, и сохранённый пробел в ширину ВХОДИТ (`pre-wrap-017`:
            // коробка `width: max-content` выходила на знак уже).
            let measured =
                if self.spaces_are_content() || (limit.is_none() && self.wrap.keep_spaces) {
                    at
                } else {
                    self.hang_tail(start, at)
                };
            // Свисающее за края в ширину строки не входит — ни открывающий
            // знак в начале, ни точка с запятой в конце.
            let head = start + self.hang_first(start);
            // Свисает ли знак — зависит от того, влезает ли строка БЕЗ него;
            // поэтому ширина считается дважды: сначала без свисания.
            let bare = self.span(&segs, head, measured);
            let tight = limit.is_some_and(|w| bare > w);
            let tail_hang = self.hang_last(measured, at >= part.end, tight);
            let mut width = self.span(&segs, head, measured - tail_hang)
                - self.tail_spacing(measured - tail_hang);
            // Строка, кончающаяся мягким переносом, несёт ещё и знак переноса.
            if self.text[..measured].ends_with('\u{00ad}') {
                width += self.hyphen_w.get();
            }
            // Допуск в сотую точки: ширина строки складывается из замеров
            // кусков и знака переноса, и на ТОЧНОМ совпадении с коробкой
            // накопленная ошибка решала исход (`hyphens-manual-011`: строка,
            // влезающая ровно, уходила на перенос).
            let over = limit.is_some_and(|w| f32::from(width) > f32::from(w) + 0.01);
            // Обязательный разрыв проверяется ПОСЛЕ переполнения: до него
            // строка может не влезать, и тогда сперва переносится она.
            // Раньше кусок перед переводом строки уходил в строку целиком,
            // сколько бы ни переполнял коробку (`pre-wrap-leading-spaces`).
            if mandatory && !over {
                // Хвост `pre-wrap` перед принудительным разрывом висит
                // УСЛОВНО: влезшая часть занимает место (css-text-3 §4.1.3).
                let width = self.conditional_width(segs, head, at, width, limit);
                out.push(Line {
                    range: start..at,
                    width,
                    ellipsis: false,
                    hyphen: false,
                    indent: ind,
                });
                start = bol(at);
                // За жёстким разрывом начинается новый кусок: при `each-line`
                // отступ повторяется, но «первым куском блока» он уже не будет.
                head_of_part = true;
                first_part = false;
                last_fit = None;
                i += 1;
                continue;
            }
            if over {
                // Переносим по последней подошедшей точке; если её нет —
                // рвём по знакам, но только когда это разрешено.
                let cut = last_fit.filter(|c| *c > start).unwrap_or_else(|| {
                    // Разрешение рвать слово берётся ПО МЕСТУ переполнения:
                    // `overflow-wrap` на вложенном `<span>` действует только
                    // на его знаки.
                    // Разрешение берётся ПО МЕСТУ, где строка переполнилась,
                    // а не по её началу: `overflow-wrap` на вложенном
                    // `<span>` действует на свои знаки, и кусок этот обычно
                    // начинается посреди строки
                    // (`overflow-wrap-anywhere-inline-*`).
                    // `white-space: nowrap` запрещает и аварийный разрыв:
                    // `overflow-wrap` действует, только когда перенос вообще
                    // разрешён (`overflow-wrap-002`).
                    // …и ВНУТРИ строки тоже: `<span>` с `overflow-wrap:
                    // anywhere` посреди неразрывного ряда не касается ни его
                    // начала, ни конца (`overflow-wrap-anywhere-inline-002/004`:
                    // ряд «X<span>XX</span>XX» уходил одной строкой за край).
                    if self.emergency_ok(start)
                        || self.emergency_ok(at.saturating_sub(1))
                        || self.emergency_inside(start, at)
                    {
                        self.cut_by_char(start, at, limit, &x)
                    } else {
                        at
                    }
                });
                let tail = if self.spaces_are_content() {
                    cut
                } else {
                    self.hang_tail(start, cut)
                };
                let tail = tail - self.hang_last(tail, false, true);
                // Разрыв по мягкому переносу: на строке остаётся знак
                // переноса, и он же входит в её ширину.
                let hyphen = self.text[..cut].ends_with('\u{00ad}');
                let extra = if hyphen { self.hyphen_w.get() } else { px(0.) };
                out.push(Line {
                    range: start..self.drop_collapsible_tail(start, cut),
                    width: self.span(&segs, head, tail) - self.tail_spacing(tail) + extra,
                    ellipsis: false,
                    hyphen,
                    indent: ind,
                });
                start = bol(cut);
                // Мягкий перенос кусок не кончает: следующая строка отступа
                // не получает (кроме `hanging`, где его получают именно они).
                head_of_part = false;
                last_fit = None;
                // Ту же точку проверяем заново от нового начала строки: за
                // одним переносом может идти следующий.
                continue;
            }
            last_fit = Some(at);
            i += 1;
        }
        if start < part.end || out.is_empty() {
            let end = part.end;
            // Тот же довод, что и в цикле: висеть пробелу можно только за
            // КРАЕМ, а при замере по максимальному содержимому края нет
            // (`pre-wrap-017`).
            let tail = if self.spaces_are_content() || (limit.is_none() && self.wrap.keep_spaces) {
                end
            } else {
                self.hang_tail(start, end)
            };
            let head = start + self.hang_first(start);
            let tail = tail - self.hang_last(tail, true, true);
            let (fl, fr) = self.flow_cut(out.len());
            let indent = self.indent_of(head_of_part, first_part, limit) + px(fl);
            // Конец блока — тоже принудительный разрыв: хвост `pre-wrap`
            // последней строки висит условно (`pre-wrap-019`, `#test2`:
            // `"0 "` занимает 2ch, а не 1ch).
            let room = limit.map(|w| w - indent - px(fr));
            let bare = self.span(&segs, head, tail) - self.tail_spacing(tail);
            out.push(Line {
                range: start..end,
                width: self.conditional_width(segs, head, end, bare, room),
                ellipsis: false,
                hyphen: false,
                indent,
            });
        }
        // Печать разреза строк: `HTML_LINES=1`. Себя окупила — ею нашлось,
        // что узел из идеографических пробелов не доезжает до раскладки
        // ВООБЩЕ (отбрасывался разбором). Когда след ведёт «строка пропала»,
        // смотреть надо сюда, а не в саму раскладку.
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("HTML_LINES").is_ok());
            *ON
        } {
            eprintln!(
                "LINES fonts={:?} {:?} -> {:?}",
                self.runs
                    .iter()
                    .map(|r| r.font.family.to_string())
                    .collect::<Vec<_>>(),
                self.text,
                out.iter()
                    .map(|l| (l.range.clone(), f32::from(l.width)))
                    .collect::<Vec<_>>()
            );
        }
        out
    }

    /// Ширина строки перед ПРИНУДИТЕЛЬНЫМ разрывом (конец блока — тоже он) с
    /// учётом условного висения, css-text-3 §4.1.3 шаг 4: «If white-space is
    /// set to pre-wrap, the UA must (unconditionally) hang this sequence,
    /// unless the sequence is followed by a forced line break, in which case
    /// it must conditionally hang the sequence instead». Условно висящее
    /// входит в ширину, пока влезает. Висящие без условий знаки перед ним
    /// (U+3000 при `normal`) висят, только если условный ряд начинается уже
    /// НЕ раньше края (`hanging-whitespace-003`: строки с рядом от 6, 5 и 4ch
    /// в коробке 4ch висят целиком, ряд от 3ch занимает место).
    fn conditional_width(
        &self,
        segs: &[Seg],
        head: usize,
        end: usize,
        bare: Pixels,
        room: Option<Pixels>,
    ) -> Pixels {
        let Some(room) = room else { return bare };
        if self.spaces_are_content() || head >= end {
            return bare;
        }
        let body = self.text[head..end]
            .trim_end_matches(['\n', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}']);
        let body_end = head + body.len();
        // Начало хвостового ряда СОХРАНЁННЫХ пробелов переносящего куска.
        let mut from = body_end;
        for (i, ch) in body.char_indices().rev() {
            let w = self.wrap_at(head + i);
            if matches!(ch, ' ' | '\t') && w.keep_spaces && !w.nowrap && !w.break_spaces {
                from = head + i;
            } else {
                break;
            }
        }
        if from == body_end || self.span(segs, head, from) >= room {
            return bare;
        }
        let full = self.span(segs, head, body_end) - self.tail_spacing(body_end);
        let fit = if full < room { full } else { room };
        if fit > bare { fit } else { bare }
    }

    /// Место разрыва внутри неразрывного куска — по знакам, до последнего
    /// влезающего.
    fn cut_by_char(
        &self,
        start: usize,
        end: usize,
        limit: Option<Pixels>,
        x: &dyn Fn(usize) -> Pixels,
    ) -> usize {
        let Some(limit) = limit else { return end };
        let from = x(start);
        let mut last = start;
        // Конец отрезка — тоже граница знака, и проверять его ОБЯЗАТЕЛЬНО:
        // без него разрез, у которого не влезал только последний знак,
        // возвращал весь отрезок целиком. При `break-spaces` это съедало
        // ведущий пробел следующей строки — он уезжал в конец предыдущей.
        let bounds = self.text[start..end]
            .char_indices()
            .map(|(i, _)| start + i)
            .chain(std::iter::once(end));
        for at in bounds {
            if at == start {
                continue;
            }
            // Рвать ВНУТРИ грозди знаков нельзя: огласовка, соединитель и
            // знак вариации принадлежат своей букве и в другую строку не
            // уходят (`overflow-wrap-cluster`: देवनागरी рвалась пополам).
            if !cluster_edge(&self.text, at) {
                continue;
            }
            // И только там, где аварийный разрыв РАЗРЕШЁН: у соседнего куска
            // правила могут быть другими.
            if !self.emergency_ok(at.saturating_sub(1)) && !self.emergency_ok(at) {
                continue;
            }
            // `word-break: break-all` рвёт между БУКВАМИ и запретов типографики
            // не отменяет: перед точкой и после знака-приставки строка не
            // рвётся даже в аварийном разрезе (`word-break-break-all-inline-008`
            // — «X» и «.» обязаны остаться вместе и вылезти за коробку).
            // Семейство `anywhere` — наоборот, рвёт где угодно.
            if !self.loose_at(at.saturating_sub(1)) && !self.loose_at(at) {
                let after = self.text[at..].chars().next();
                let before = self.text[..at].chars().next_back();
                if after.is_some_and(no_break_before) || before.is_some_and(no_break_after) {
                    continue;
                }
            }
            if x(at) - from > limit {
                return if last > start { last } else { at };
            }
            last = at;
        }
        // Хвост за последней разрешённой точкой не влез — разрыв по ней.
        // Прежде возвращался весь отрезок: ряд «XX<span>XX</span>XXX» с
        // `overflow-wrap: anywhere` на `<span>` рвался внутри него, но
        // последняя точка (между `<span>` и хвостом) терялась, и хвост
        // уезжал за край вместе с частью `<span>`.
        if last > start && x(end) - from > limit {
            return last;
        }
        end
    }

    /// Есть ли между `start` и `end` кусок с разрешённым аварийным разрывом
    /// (см. `emergency_ok`).
    fn emergency_inside(&self, start: usize, end: usize) -> bool {
        self.spans.iter().any(|(r, w)| {
            r.start < end
                && r.end > start
                && !w.nowrap
                && (w.break_all || w.anywhere || w.break_word || w.wrap_anywhere)
        })
    }

    /// Конец измеряемой части строки: висящий хвост срезается ПО МЕСТУ.
    ///
    /// Пробел куска с `break-spaces` (и `pre`) не висит (css-text-3 §4.1.3:
    /// «treated the same as other visible characters»), а висеть может только
    /// то, что стоит у самого края, — значит, и всё ПЕРЕД ним остаётся в
    /// строке (`hanging-whitespace-001`: U+3000 абзаца `normal` перед
    /// `<span style="white-space:break-spaces"> </span>`). `spaces_are_content`
    /// смотрит правило абзаца и вложенного куска не видит. Без таких кусков
    /// результат совпадает с `trim_hanging`.
    fn hang_tail(&self, start: usize, end: usize) -> usize {
        let mut at = end;
        for (i, ch) in self.text[start..end].char_indices().rev() {
            if ch == '\u{feff}' || !(hangs(ch) || zero_width(ch)) {
                break;
            }
            if ch != '\n' && hangs(ch) {
                let w = self.wrap_at(start + i);
                if w.break_spaces || (w.keep_spaces && w.nowrap) {
                    break;
                }
            }
            at = start + i;
        }
        at
    }

    /// Неперносима ли точка МЕЖДУ двумя знаками.
    ///
    /// Решает её общий предок (css-text-3 §5.1). У нас предки выражены
    /// диапазонами кусков: если оба знака в ОДНОМ куске, правило его; если в
    /// разных (или один вне кусков) — общий предок это сам абзац.
    fn nowrap_between(&self, at: usize) -> bool {
        let which = |i: usize| self.spans.iter().position(|(r, _)| r.contains(&i));
        let left = which(at.saturating_sub(1));
        let right = which(at);
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.10): точку переноса после ПРОБЕЛА решает
        // `white-space` элемента с самим пробелом (css-text-3 §5.1, «for soft
        // wrap opportunities created by characters that disappear or are
        // preserved spaces»), а не общий предок. Срез 1973 пар (css-text,
        // CSS2/text, <pre>): +1/−3 — white-space-wrap-after-nowrap-001
        // 0.62 -> 0.28, но white-space-007 0.04 -> 11.99,
        // white-space-collapsing-breaks-001 0.00 -> «красное видно». Причина
        // не разобрана; подозрение — после схлопывания через границу куска
        // (`collapse_across_pieces`) уцелевший пробел лежит не в том куске,
        // что у браузера.
        match (left, right) {
            (Some(a), Some(b)) if a == b => self.spans[a].1.nowrap,
            _ => self.wrap.nowrap,
        }
    }

    /// Конец строки после шага 3 Phase II (css-text-3 §4.1.3): «A sequence of
    /// collapsible spaces at the end of a line … is removed». Удаляется из
    /// СТРОКИ, а не только из её ширины: подложка куска больше не тянется по
    /// пробелу (`line-break-anywhere-and-white-space-004`). Схлопываемые —
    /// только U+0020 и табуляция там, где пробелы не сохраняются; U+3000,
    /// U+00A0 и прочие Zs не схлопываются, они ВИСЯТ и рисуются (★ откат у
    /// `trim_hanging`: обрезка подложки по всему `hangs` ломала
    /// `trailing-ideographic-space-*`).
    fn drop_collapsible_tail(&self, start: usize, end: usize) -> usize {
        let mut at = end;
        for (i, ch) in self.text[start..end].char_indices().rev() {
            if matches!(ch, ' ' | '\t') && !self.wrap_at(start + i).keep_spaces {
                at = start + i;
            } else {
                break;
            }
        }
        at
    }

    /// Рвётся ли на этом месте что угодно и где угодно — без оглядки на
    /// типографику (`line-break: anywhere`, `overflow-wrap: anywhere`,
    /// `word-wrap: break-word`).
    fn loose_at(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        w.anywhere || w.break_word || w.wrap_anywhere
    }

    /// Разрешён ли на этом месте аварийный разрыв по знакам.
    fn emergency_ok(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        !w.nowrap && (w.break_all || w.anywhere || w.break_word || w.wrap_anywhere)
    }

    /// Точки, где строку РАЗРЕШЕНО разорвать.
    /// Правила переноса, действующие на байте `at`: сначала свой кусок, потом
    /// абзац целиком.
    /// Сохранённые пробелы конца строки — СОДЕРЖИМОЕ, а не висящие: при
    /// `break-spaces` (они занимают место и дают разрыв) и при `pre`
    /// (css-text-3 §4.1.3 висят только `normal`/`nowrap`/`pre-line` — без
    /// условий — и `pre-wrap` — условно; `pre` в списке нет:
    /// `white-space-intrinsic-size-015`, эталон `eol-spaces-bidi-004`).
    fn spaces_are_content(&self) -> bool {
        self.wrap.break_spaces || (self.wrap.keep_spaces && self.wrap.nowrap)
    }

    fn wrap_at(&self, at: usize) -> Wrap {
        self.spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, w)| *w)
            .unwrap_or(self.wrap)
    }

    /// Точки переноса по UAX-14 — по тексту БЕЗ знаков-распорок.
    ///
    /// Распорка (`inline::SPACER`) — не знак документа, а место под поля
    /// строчной коробки. Класс WJ запрещает разрыв и перед собой, поэтому
    /// пробел ПЕРЕД `<span>` с отступом переставал быть точкой переноса, и
    /// строка уходила за край коробки вместо переноса. Разрыв возвращается на
    /// место распорки: поле уезжает на новую строку вместе со своим текстом.
    fn linebreaks(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        let mut out = self.linebreaks_uax();
        // Атом — точка переноса с обеих сторон (css-text-3 §5.1: для переноса
        // атом — знак-заместитель объекта; Blink `line_breaker.cc` рвёт до и
        // после атомарной коробки). Нельзя только рядом со знаками GL/WJ/ZWJ
        // — «with the exception of U+00A0 NO-BREAK SPACE» (§5.1
        // «atomic-compat-wrap»): рядом с ним разрыв, наоборот, есть
        // (`line-breaking-atomic-001/002`). Рядом с пробелом точку даёт сам
        // UAX #14 — после ряда пробелов, а не перед ним.
        if !self.atom_boxes.is_empty() {
            let glue = |ch: char| {
                use unicode_linebreak::BreakClass::*;
                ch != '\u{a0}'
                    && matches!(
                        unicode_linebreak::break_property(ch as u32),
                        NonBreakingGlue | WordJoiner | ZeroWidthJoiner
                    )
            };
            // Пунктуация разрыв у атома НЕ гасит: «there is a soft wrap
            // opportunity before and after each replaced element or other
            // atomic inline, even when adjacent to a character that would
            // normally suppress them» (css-text-3 §5.1;
            // `line-breaking-replaced-006`: `<img>:` рвётся перед двоеточием).
            let space = |ch: char| matches!(ch, ' ' | '\t' | '\n' | '\u{200b}');
            // Соседний знак — мимо распорок полей (их перенос не видит, см.
            // `linebreaks_uax`); соседний атом читается знаком-заместителем.
            let atom_at = |at: usize| self.atom_boxes.iter().any(|x| x.at == at);
            let skip = |at: usize| self.spacers.binary_search(&at).is_ok();
            let prev_of = |mut at: usize| -> Option<char> {
                loop {
                    let (i, ch) = self.text[..at].char_indices().next_back()?;
                    if atom_at(i) {
                        return Some('\u{fffc}');
                    }
                    if !skip(i) {
                        return Some(ch);
                    }
                    at = i;
                }
            };
            let next_of = |mut at: usize| -> Option<char> {
                loop {
                    let ch = self.text.get(at..)?.chars().next()?;
                    if atom_at(at) {
                        return Some('\u{fffc}');
                    }
                    if !skip(at) {
                        return Some(ch);
                    }
                    at += ch.len_utf8();
                }
            };
            for b in &self.atom_boxes {
                let end = b.at + 3;
                if let Some(prev) = prev_of(b.at)
                    && !space(prev)
                    && !glue(prev)
                {
                    out.push((b.at, unicode_linebreak::BreakOpportunity::Allowed));
                }
                if let Some(next) = next_of(end)
                    && !space(next)
                    && !glue(next)
                {
                    out.push((end, unicode_linebreak::BreakOpportunity::Allowed));
                }
            }
            out.sort_by_key(|(at, _)| *at);
            out.dedup_by_key(|(at, _)| *at);
        }
        out
    }

    fn linebreaks_uax(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        // Распорка атома читается как U+FFFC: класс CB даёт разрыв до и после
        // (UAX #14 LB20; css-text-3 §5.1 — для переноса атом как знак-
        // заместитель объекта, Blink `inline_items_builder.cc`). Длина в
        // UTF-8 у U+FEFF и U+FFFC одна — смещения не съезжают.
        let replaced;
        let text: &str = if self.atom_boxes.is_empty() {
            &self.text
        } else {
            let mut t = self.text.to_string();
            for b in &self.atom_boxes {
                if t.get(b.at..b.at + 3) == Some("\u{feff}") {
                    t.replace_range(b.at..b.at + 3, "\u{fffc}");
                }
            }
            replaced = t;
            &replaced
        };
        if self.spacers.is_empty() {
            return unicode_linebreak::linebreaks(text).collect();
        }
        let mut clean = String::with_capacity(text.len());
        let mut map: Vec<usize> = Vec::with_capacity(text.len() + 1);
        let mut pending: Option<usize> = None;
        for (at, ch) in text.char_indices() {
            if self.spacers.binary_search(&at).is_ok() {
                pending.get_or_insert(at);
                continue;
            }
            map.push(pending.take().unwrap_or(at));
            for k in 1..ch.len_utf8() {
                map.push(at + k);
            }
            clean.push(ch);
        }
        map.push(self.text.len());
        unicode_linebreak::linebreaks(&clean)
            .map(|(at, kind)| (map.get(at).copied().unwrap_or(self.text.len()), kind))
            .collect()
    }

    fn opportunities(&self) -> Vec<Stop> {
        let mut out: Vec<Stop> = Vec::new();
        // Обязательные разрывы есть всегда, даже при `nowrap`.
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                out.push(Stop {
                    at: i + 1,
                    mandatory: true,
                });
            }
        }
        {
            if self.wrap.anywhere {
                for (i, _) in self.text.char_indices().skip(1) {
                    out.push(Stop {
                        at: i,
                        mandatory: false,
                    });
                }
            } else {
                // `line-break: anywhere` на вложенном куске: точки ставятся
                // только внутри него, остальной абзац живёт по UAX-14.
                for (range, w) in &self.spans {
                    if !w.anywhere {
                        continue;
                    }
                    for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                        out.push(Stop {
                            at: range.start + i,
                            mandatory: false,
                        });
                    }
                }
                for (at, kind) in self.linebreaks() {
                    // Обязательные разрывы Юникода — это не только перевод
                    // строки: подача страницы, вертикальная табуляция,
                    // разделители строки и абзаца, NEL. Все они заканчивают
                    // строку принудительно (`line-breaking-022`).
                    match kind {
                        unicode_linebreak::BreakOpportunity::Allowed => out.push(Stop {
                            at,
                            mandatory: false,
                        }),
                        // Конец текста переносчик тоже зовёт обязательным
                        // разрывом — но переносить там нечего, а лишняя точка
                        // ломает счёт строк.
                        unicode_linebreak::BreakOpportunity::Mandatory if at < self.text.len() => {
                            out.push(Stop {
                                at,
                                mandatory: true,
                            })
                        }
                        unicode_linebreak::BreakOpportunity::Mandatory => {}
                    }
                }
                {
                    // Разрыв разрешён между знаками слова, но запреты
                    // типографики он не отменяет: перед точкой, скобкой или
                    // знаком препинания рвать всё равно нельзя (это отличает
                    // `break-all` от `line-break: anywhere`). Правило берётся
                    // ПО МЕСТУ: заданное на вложенном `<span>`, оно действует
                    // только на его байты.
                    for (i, ch) in self.text.char_indices().skip(1) {
                        if !self.wrap_at(i).break_all {
                            continue;
                        }
                        let before = self.text[..i].chars().next_back();
                        let allowed = !ch.is_whitespace()
                            && !no_break_before(ch)
                            && before.is_none_or(|c| !no_break_after(c));
                        if allowed {
                            out.push(Stop {
                                at: i,
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // `line-break: normal`/`loose` (css-text-3 §5.2): перед
                    // малой каной и знаком долготы (класс CJ) разрыв
                    // разрешён — UAX-14 в строгом варианте держит их при
                    // предыдущем знаке. В `loose` ещё перед знаками повтора и
                    // между неразделимыми `‥…` (класс IN). Слева — не пробел
                    // и не открывающая скобка/запрет (`line-break-loose-011`,
                    // `line-break-normal-011`). Уровень берётся ПО МЕСТУ.
                    for (i, ch) in self.text.char_indices().skip(1) {
                        let level = self.wrap_at(i).loose;
                        if level == 0 {
                            continue;
                        }
                        let Some(before) = self.text[..i].chars().next_back() else {
                            continue;
                        };
                        let cjk = self.wrap_at(i).cjk_lang;
                        let eased = conditional_japanese_starter(ch)
                            || (level >= 2
                                && (iteration_mark(ch)
                                    || (matches!(ch, '\u{2025}' | '\u{2026}')
                                        && matches!(before, '\u{2025}' | '\u{2026}'))))
                            // Только для китайского/японского письма (§5.2):
                            // `normal`/`loose` — перед волнистым тире
                            // U+301C/U+30A0; `loose` — перед центрированной
                            // пунктуацией, перед широкими постфиксами (PO)
                            // и ПОСЛЕ широких префиксов (PR)
                            // (`line-break-loose-016a/016b/017a/017b/018`).
                            || (cjk && matches!(ch, '\u{301C}' | '\u{30A0}'))
                            || (cjk
                                && level >= 2
                                && (centered_punctuation(ch) || wide_postfix(ch)));
                        // После широкого префикса запрет UAX-14 «PR × ID»
                        // снимается целиком — проверяется только правый знак.
                        let after_prefix = cjk
                            && level >= 2
                            && wide_prefix(before)
                            && !ch.is_whitespace()
                            && !no_break_before(ch);
                        if after_prefix
                            || (eased
                                && !before.is_whitespace()
                                && !no_break_after(before)
                                && !matches!(before, '\u{200B}' | '\u{2060}' | '\u{00A0}'))
                        {
                            out.push(Stop {
                                at: i,
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // Мягкий перенос — ЯВНАЯ точка переноса: она сильнее
                    // запретов типографики. UAX-14 держит вместе перенос и
                    // следующую за ним кавычку (`hyphens-i18n-manual-003`:
                    // «tú­’àn» не рвалось вовсе).
                    for (i, ch) in self.text.char_indices() {
                        if ch == SOFT_HYPHEN {
                            out.push(Stop {
                                at: i + ch.len_utf8(),
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // После КАЖДОГО сохранённого пробела — своя точка разрыва.
                    // Табуляция тоже пробел: `break-spaces` рвёт и после неё,
                    // хотя UAX-14 держит подряд идущие табуляции вместе
                    // (`break-spaces-tab-003`).
                    // Пробел здесь — ЛЮБОЙ сохранённый пробельный знак:
                    // `break-spaces` рвёт и после идеографического U+3000,
                    // и после em-space U+2003 (`break-spaces-with-ideographic-
                    // space-005/010`, `trailing-ideographic-space-break-spaces-007`
                    // показывали красное). Перевод строки исключён: его разрыв
                    // обязательный и ставится своим проходом.
                    for (i, ch) in self.text.char_indices() {
                        if ch.is_whitespace()
                            && ch != '\n'
                            && ch != '\r'
                            && self.wrap_at(i).break_spaces
                        {
                            out.push(Stop {
                                at: i + ch.len_utf8(),
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // Запрет действует только МЕЖДУ буквенными единицами:
                    // иероглифы друг от друга не отрываются, а после запятой
                    // или дефиса строка рвётся по-прежнему.
                    out.retain(|s| {
                        if !self.wrap_at(s.at).keep_all {
                            return true;
                        }
                        let before = self.text[..s.at].chars().next_back();
                        let after = self.text[s.at..].chars().next();
                        s.mandatory
                            || !(before.is_some_and(letter_unit) && after.is_some_and(letter_unit))
                    });
                }
            }
        }
        // `white-space: nowrap`/`pre` гасит МЯГКИЕ точки — но по месту, а не по
        // абзацу целиком: вложенный `<span>` со своим `white-space` переносится
        // внутри неперносимого абзаца и наоборот (`white-space-pre-031`).
        //
        // Точку между ДВУМЯ знаками решает их общий предок (css-text-3
        // §5.1), поэтому гасится она, только если неперносимы ОБЕ стороны.
        // Пока смотрели один знак слева, `<span style="white-space:pre">口</span>口`
        // не рвался на границе куска, хотя рвать там велит div-родитель
        // (`line-breaking-ic-001`).
        out.retain(|s| s.mandatory || !self.nowrap_between(s.at));
        // Внутри грозди знаков рвать нельзя НИКОГДА: составной знак (флаг,
        // смайлик с модификатором) — одна буква, и переносчик UAX-14 о его
        // устройстве не знает (`line-breaking-014`: радужный флаг рассыпался
        // на четыре строки).
        // Обязательный разрыв не снимается НИКОГДА: перевод строки обязан
        // закончить строку, иначе набор получает строку с переводом внутри и
        // падает на проверке (`text argument should not contain newlines`).
        // `line-break: anywhere` рвёт между ЛЮБЫМИ знаками, включая склеенные
        // соединителем нулевой ширины: класс ZWJ он тоже перекрывает
        // (`line-break-anywhere-overrides-uax-behavior-015`).
        out.retain(|s| {
            s.mandatory || cluster_edge_at(&self.text, s.at, self.wrap_at(s.at).anywhere)
        });
        // Знак перед числом держит следующий за собой: `$`, `£`, `\`. Таблица
        // пар UAX-14 в переносчике этого не знает и рвёт «XX XX\\\» между
        // обратными косыми (`word-break-break-all-023`), тогда как рвать
        // разрешено только ПЕРЕД первой из них.
        // `line-break: anywhere` и `overflow-wrap: anywhere` снимают запреты
        // типографики целиком — их точки остаются.
        // `line-break: loose` в китайском/японском письме снимает запрет и
        // после ШИРОКИХ префиксов (css-text-3 §5.2: «breaks after prefixes
        // (PR) with East Asian Width A/F/W», `line-break-loose-018`).
        out.retain(|s| {
            let w = self.wrap_at(s.at);
            // Распорка атома (U+FEFF, класс WJ) здесь не запрет: за атомом
            // точку ставит `linebreaks`.
            s.mandatory
                || w.anywhere
                || w.wrap_anywhere
                || self
                    .atom_boxes
                    .iter()
                    .any(|b| b.at + 3 == s.at || b.at == s.at)
                || self.text[..s.at].chars().next_back().is_none_or(|c| {
                    !no_break_after(c) || (w.loose >= 2 && w.cjk_lang && wide_prefix(c))
                })
        });
        out.sort_by_key(|s| (s.at, !s.mandatory));
        out.dedup_by_key(|s| s.at);
        out
    }
}

/// Сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку куска: `Some(true)` — справа
/// налево, `None` — сильных знаков нет вовсе.
fn first_strong_rtl(text: &str) -> Option<bool> {
    for ch in text.chars() {
        match unicode_bidi::bidi_class(ch) {
            unicode_bidi::BidiClass::L => return Some(false),
            unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL => return Some(true),
            _ => {}
        }
    }
    None
}

/// Открывающий знак — скобка или кавычка.
fn is_opening(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | Quotation
    )
}

/// Закрывающий знак — скобка или кавычка.
fn is_closing(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation | CloseParenthesis | Quotation
    )
}

/// Точка или запятая — то, что свисает по `force-end`/`allow-end`.
fn is_stop(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | '\u{060C}'
            | '\u{06D4}'
            | '、'
            | '。'
            | '，'
            | '．'
            | '\u{FE50}'
            | '\u{FE51}'
            | '\u{FE52}'
            | '\u{FF61}'
            | '\u{FF64}'
    )
}

/// Буквенная единица письма: между такими знаками `word-break: keep-all`
/// запрещает разрыв. Знаки препинания сюда не входят — после них рвать можно.
///
/// Пробел единицей письма не является НИКАКОЙ, даже идеографический: по
/// классу переноса он иероглиф (ID), и запрет заодно снимал перенос по нему
/// (`word-space-transform-013`: коробка шла одной строкой за край).
fn letter_unit(ch: char) -> bool {
    if ch.is_whitespace() {
        return false;
    }
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        Alphabetic
            | Numeric
            | Ambiguous
            | Ideographic
            | ConditionalJapaneseStarter
            | HebrewLetter
            | ComplexContext
            | CombiningMark
            | HangulLvSyllable
            | HangulLvtSyllable
            | HangulLJamo
            | HangulVJamo
            | HangulTJamo
    )
}

/// Знак, перед которым рвать нельзя: закрывающая скобка, знак препинания,
/// разделитель разрядов, неразрывный пробел (классы UAX-14).
fn no_break_before(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation
            | CloseParenthesis
            | Exclamation
            | InfixSeparator
            | NonStarter
            | Symbol
            | NonBreakingGlue
            | WordJoiner
            | ZeroWidthJoiner
    )
}

/// Знак, после которого рвать нельзя: открывающая скобка, склейка, знак
/// перед числом (`$`, `\`, `£`).
///
/// Соединитель нулевой ширины держит составные знаки вместе — на нём собраны
/// целые эмодзи (человек + компьютер = «программист»). Разрыв по нему
/// рассыпал бы один знак на составные части.
fn no_break_after(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | NonBreakingGlue | WordJoiner | ZeroWidthJoiner | Prefix
    )
}

/// Забыть замеры. Зовётся при разборе новой страницы: одно и то же имя
/// семейства на разных страницах означает РАЗНЫЕ шрифты (`@font-face`), и
/// старые положения знаков стали бы чужими.
pub fn forget_measures() {
    MEASURED.with(|c| c.borrow_mut().clear());
    SPLITS.with(|c| c.borrow_mut().clear());
}

thread_local! {
    /// Память разрезов: ключ разреза → готовые строки.
    static SPLITS: std::cell::RefCell<
        std::collections::HashMap<u64, std::rc::Rc<Vec<Line>>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Сколько замеров абзацев помнить между кадрами.
const MEASURE_CACHE: usize = 64;

/// Память подбора кегля: ключ абзаца → найденный множитель.
fn remember_fit(key: u64, k: f32) {
    FITTED.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some(hit) = cache.iter_mut().find(|(hit, _)| *hit == key) {
            hit.1 = k;
            return;
        }
        if cache.len() >= MEASURE_CACHE {
            cache.remove(0);
        }
        cache.push((key, k));
    });
}

thread_local! {
    /// Найденные множители `text-fit`: ключ абзаца → во сколько раз крупнее.
    static FITTED: std::cell::RefCell<Vec<(u64, f32)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Память замеров: ключ стиля и текста → положения знаков по кускам.
    static MEASURED: std::cell::RefCell<Vec<(u64, Vec<Seg>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Знак-указание, у которого нет своего изображения: словосоединитель,
/// нулевой пробел, метка порядка байтов, мягкий перенос, знаки управления
/// встроенностью.
fn invisible(ch: char) -> bool {
    matches!(ch as u32,
        0x00AD | 0x200B | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// Прогоны без невидимых знаков: длины считаются по оставшимся байтам.
fn trim_runs(runs: &[TextRun], text: &str) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len());
    let mut at = 0usize;
    for run in runs {
        let end = (at + run.len).min(text.len());
        let kept: usize = text
            .get(at..end)
            .map(|s| {
                s.chars()
                    .filter(|c| !invisible(*c))
                    .map(char::len_utf8)
                    .sum()
            })
            .unwrap_or(0);
        if kept > 0 {
            let mut piece = run.clone();
            piece.len = kept;
            out.push(piece);
        }
        at = end;
    }
    out
}

/// Слово строки и сколько пробелов стоит перед ним от начала строки.
struct Word {
    range: std::ops::Range<usize>,
    spaces_before: usize,
}

/// Точка возможного разрыва.
#[derive(Clone, Copy, Debug)]
struct Stop {
    at: usize,
    mandatory: bool,
}

/// Какой край отрезка спрашивают: у конца строки индекс сразу за переводом
/// строки принадлежит прошлому куску, у начала — новому.
#[derive(Clone, Copy, PartialEq)]
enum Edge {
    Start,
    End,
}

/// Кусок текста между обязательными разрывами вместе со своим набором.
#[derive(Clone)]
struct Seg {
    start: usize,
    end: usize,
    layout: std::sync::Arc<gpui::LineLayout>,
    /// Сдвиг начала куска внутри своей строки. Нужен табуляции: она рвёт
    /// набор на куски, и каждый следующий начинается со своей позиции табуляции.
    offset: Pixels,
}

/// Длина куска без хвостовых пробелов — они висят за краем строки.
/// Сколько байт схлопываемых пробелов в НАЧАЛЕ строки: по CSS они удаляются
/// вместе с переносом, иначе следующая строка начинается с отступа в пробел.
fn skip_leading(chunk: &str) -> usize {
    chunk.len() - chunk.trim_start_matches([' ', '\t']).len()
}

/// Разделитель, который висит за краем строки. Неразрывный пробел сюда НЕ
/// входит: он держит слова вместе и место занимает всегда. Идеографический
/// U+3000 и прочие Zs-разделители ВИСЯТ (`trailing-ideographic-space-002`,
/// `trailing-other-space-separators-001..004`) — сужение набора до
/// 0x20/09/0A теряло 9 зелёных пар, а целевые break-spaces тесты не чинило.
fn hangs(ch: char) -> bool {
    matches!(
        ch as u32,
        0x20 | 0x09 | 0x0A | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000
    )
}

/// Разделитель слов по css-text-3 §8.2: к нему прибавляется `word-spacing`,
/// и по нему же выключка раздаёт остаток строки.
///
/// Обычным пробелом набор не исчерпывается: пока неразрывный в него не
/// входил, `word-spacing` на строке из `&nbsp;` не действовал вовсе
/// (`word-spacing-001`).
fn word_separator(ch: char) -> bool {
    // Идеографический пробел U+3000 — ФИКСИРОВАННОЙ ширины и разделителем
    // слов НЕ считается (css-text-3 §word-separator; word-spacing-
    // characters-001): `word-spacing` его не трогает.
    matches!(
        ch as u32,
        0x20 | 0xA0 | 0x1361 | 0x10100 | 0x10101 | 0x1039F | 0x1091F
    )
}

/// Знак управления нулевой ширины: сам не висит, но обрезка хвоста смотрит
/// сквозь него — иначе пробел перед ним перестаёт висеть.
fn zero_width(ch: char) -> bool {
    matches!(ch as u32, 0x200B | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО: обрезать ПОДЛОЖКУ прогонов на висящем хвосте
/// строки (css-text §8.1: разделители у конца строки в строку не входят,
/// значит и фон строчной коробки за ними тянуться не должен). Набор при этом
/// рисовался целиком, менялись только `background_color` хвостовых прогонов.
/// Срез css-text + white-space + linebox (1730 пар, 1582 зелёных):
/// * хвост по всему `hangs` — 1573 (−9): три целевых
///   `trailing-other-space-separators-001/003/004` 0.54 → 0.00, но вся семья
///   `trailing-ideographic-space-002/017..025` уходит в «красное видно»:
///   идеографический пробел U+3000 фиксированной ширины и НЕ висит;
/// * без U+3000 — 1580 (−2): целевые три возвращаются в красное (их хвост
///   КОНЧАЕТСЯ на U+3000, обрезка об него спотыкается), ломаются
///   `line-break-anywhere-and-white-space-006/007` (`pre-wrap`) и
///   `word-spacing-characters-002`;
/// * без U+3000 и с пропуском строк, где пробелы СОХРАНЯЮТСЯ (`pre`,
///   `pre-wrap`), — 1582, ровно baseline: +`line-break-anywhere-and-white-
///   space-004`, −`word-spacing-characters-002`.
/// То есть висение U+3000 требуют одни пары и запрещают другие: развилка не
/// в знаке, а в том, чем кончается строка. Возвращать вместе с настоящим
/// правилом Phase II (обрезка хвоста в САМОМ разборе строки, а не в подложке).
fn trim_hanging(chunk: &str) -> usize {
    // Идеографический пробел тоже не тянет за собой перенос: место он
    // занимает и рисуется, но строку из-за него не рвут — иначе он один
    // уезжал бы на следующую строку.
    //
    // Соединитель слов (U+FEFF) из обрезки ИСКЛЮЧЁН: им помечены распорки
    // строчных коробок, и в них лежит поле — обрезав хвостовую, коробка
    // теряла своё правое поле целиком (`word-space-transform-010`).
    chunk
        .trim_end_matches(|c| c != '\u{feff}' && (hangs(c) || zero_width(c)))
        .len()
}

impl Element for Paragraph {
    type RequestLayoutState = LayoutId;
    /// Область попадания заводится только у выделяемого абзаца.
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
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
    ) -> (LayoutId, LayoutId) {
        // Атомы раскладываются ДО замера: их ширина — продвижение распорки,
        // высота растит строку (см. `lay_atoms`).
        if !self.atoms.is_empty() {
            self.lay_atoms(window, cx);
        }
        if !self.box_spans.is_empty() {
            if self.run_metrics.len() != self.runs.len() {
                self.run_metrics = self.measure_runs(window);
            }
            if let Some((font, size)) = self.strut_run.clone() {
                let ts = window.text_system();
                let id = ts.resolve_font(&font);
                self.strut_box = (
                    f32::from(ts.ascent(id, size)),
                    f32::from(ts.descent(id, size)).abs(),
                );
            }
        }
        let atom_boxes = self.atom_boxes.clone();
        let edge_spans = self.edge_spans.clone();
        let ruby_trim = self.ruby_trim;
        let emph_spans = self.emph_spans.clone();
        let box_spans = self.box_spans.clone();
        let strut_box = self.strut_box;
        let run_metrics = self.run_metrics.clone();
        let strut = self.strut;
        // Ширина известна только раскладке, поэтому строки считаются в замере:
        // сколько дали места — столько строк и получилось.
        let text = self.text.clone();
        let runs = self.runs.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let wrap = self.wrap;
        let align = self.align;
        let vertical = self.vertical;
        let lines_reversed = self.lines_reversed;
        let vertical_central_baseline = self.vertical_central_baseline;
        let vertical_ccw = self.vertical_ccw;
        let vertical_inline = self.vertical_inline;
        let ortho_limit = self.ortho_limit;
        // Правила КУСКОВ обязаны доехать и до замера: без них щуп считает
        // абзац по общим правилам и отдаёт другое число строк, чем потом
        // рисуется. Коробка тогда выходит по замеру, а текст по отрисовке —
        // и лишние строки вылезают за рамку (`white-space-pre-031`).
        let spans = self.spans.clone();
        let word_spans = self.word_spans.clone();
        let letter_spans = self.letter_spans.clone();
        let shift_spans = self.shift_spans.clone();
        let lh_spans = self.lh_spans.clone();
        // Обрыв по `line-clamp` обязан доехать и до замера: иначе коробка
        // считается по ПОЛНОМУ числу строк, а рисуются обрезанные, и рамка
        // выходит выше текста (`text-wrap-balance-line-clamp-004`).
        let clamp = self.clamp;
        let clamp_force = self.clamp_force;
        let fit = self.fit;
        let tab_stop = self.tab_stop.clone();
        // Отступ первой строки решает и число строк, и ширину коробки —
        // без него щуп мерил абзац по чужой раскладке.
        let indent = self.indent;
        let hanging = self.hanging;
        let spacers = self.spacers.clone();
        let flow = self.flow.clone();
        let id = window.request_measured_layout_with_physical_baselines(
            gpui::Style::default(),
            move |known, available, window, _cx| {
                // Заданная ширина сильнее доступной: раскладка уже решила, в
                // какую коробку абзац ставится, и переносы считаются по ней.
                let mut probe = Paragraph::new(
                    text.clone(),
                    runs.clone(),
                    font_size,
                    line_height,
                    align,
                    wrap,
                );
                probe.vertical = vertical;
                probe.lines_reversed = lines_reversed;
                probe.vertical_central_baseline = vertical_central_baseline;
                probe.vertical_ccw = vertical_ccw;
                probe.vertical_inline = vertical_inline;
                probe.spans = spans.clone();
                probe.word_spans = word_spans.clone();
                probe.letter_spans = letter_spans.clone();
                probe.shift_spans = shift_spans.clone();
                probe.lh_spans = lh_spans.clone();
                probe.clamp = clamp;
                probe.clamp_force = clamp_force;
                probe.fit = fit;
                probe.tab_stop = tab_stop.clone();
                probe.indent = indent;
                probe.hanging = hanging;
                probe.flow = flow.clone();
                probe.atom_boxes = atom_boxes.clone();
                probe.edge_spans = edge_spans.clone();
                probe.ruby_trim = ruby_trim;
                probe.emph_spans = emph_spans.clone();
                probe.box_spans = box_spans.clone();
                probe.strut_box = strut_box;
                probe.run_metrics = run_metrics.clone();
                probe.strut = strut;
                probe.spacers = spacers.clone();
                // Предел переноса берётся ПО ОСИ СТРОКИ: по горизонтали это
                // ширина коробки, по вертикали — её высота. Уже решённая
                // родителем сторона сильнее доступной.
                let (known_along, space_along) = if vertical {
                    (known.height, available.height)
                } else {
                    (known.width, available.width)
                };
                let limit = probe.measured_inline_limit(known_along, space_along, ortho_limit, window);
                // Кегль подбирается ДО замера: коробка считается уже по
                // подобранному, иначе её высота не сойдётся с отрисовкой.
                // Подбирать есть смысл только под ЗАДАННЫЙ размер строки:
                // когда его нет, коробка растёт под текст, и заполнять нечего
                // (иначе кегль улетал в размер окна — `text-fit/writing-mode`).
                if let Some(w) = known_along {
                    probe.apply_fit(w, window);
                }
                if vertical {
                    probe.run_metrics = probe.measure_runs(window);
                }
                let lines = probe.split(limit, window);
                if {
                    static ON: std::sync::LazyLock<bool> =
                        std::sync::LazyLock::new(|| std::env::var("HTML_MEASURE").is_ok());
                    *ON
                } {
                    eprintln!(
                        "MEASURE {:?} known={:?}x{:?} avail={:?}x{:?} limit={:?} lines={:?}",
                        probe.text,
                        known.width,
                        known.height,
                        available.width,
                        available.height,
                        limit,
                        lines.iter().map(|l| f32::from(l.width)).collect::<Vec<_>>()
                    );
                }
                // Ширина — по самой длинной строке. Вся отведённая ширина
                // берётся только под выключку по ширине: там остаток строки
                // раздаётся пробелам, и без полной колонки раздавать нечего.
                // В остальных случаях абзац обтягивает текст, иначе ломается
                // размер по содержимому у родителя.
                // Отступ строки входит в её место в колонке: коробка по
                // содержимому обязана вместить и его. Отрицательный уходит в
                // поле и ширины не требует, поэтому в ноль он и упирается.
                // У абзаца с атомами отрицательный отступ ширину по содержимому
                // УМЕНЬШАЕТ (css-text-3 §7.1: отступ входит в строку; доля при
                // замере — ноль): `text-indent: calc(50% - 3px)` у флоата с
                // атомом 10px даёт 7px (`calc-text-indent-intrinsic-1`). Так
                // мерил и прежний ряд слов; у текстового абзаца — как было.
                let atoms_in = !probe.atom_boxes.is_empty();
                let content = lines
                    .iter()
                    .map(|l| {
                        if atoms_in {
                            (l.width + l.indent).max(px(0.))
                        } else {
                            l.width + l.indent.max(px(0.))
                        }
                    })
                    .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
                let width = known_along.unwrap_or(content);
                // Шире отведённого коробка не бывает: у абзаца блочного уровня
                // ширина ограничена содержащим блоком, и без этого предела
                // длинная сохранённая строка растягивала коробку и вылезала
                // за неё вместо переноса.
                let width = match space_along {
                    gpui::AvailableSpace::Definite(w) if width > w => w,
                    _ => width,
                };
                // Абзац с атомами, перенесённый МЯГКО, занимает всё отведённое
                // место: ширина «по содержимому» — это min(max-content,
                // max(min-content, доступное)) (CSS 2.1 §10.3.5, css-sizing-3
                // §5.1 fit-content), а не самая длинная строка после переноса.
                // Так мерил и прежний гибкий ряд с переносом, и коробка
                // `width: fit-content(100px)` из двух `inline-block` по 60px
                // выходила 60 вместо 100 (`fit-content-length-percentage-*`).
                let width = match space_along {
                    gpui::AvailableSpace::Definite(w)
                        if known_along.is_none() && !probe.atom_boxes.is_empty() && width < w =>
                    {
                        let full = probe
                            .split(None, window)
                            .iter()
                            .map(|l| l.width + l.indent.max(px(0.)))
                            .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
                        if full > w { w } else { width }
                    }
                    _ => width,
                };
                // Высота абзаца — сумма ШАГОВ строк: обычно это ровно
                // `line_height`, но строка со сдвинутым по вертикали куском
                // выше на его вылет (CSS 2.1 §10.8).
                // Сдвиг ПОСЛЕДНЕЙ строки от первой: шаги всех строк перед ней
                // плюс разница их надбавок сверху (у первой базовой надбавка
                // своей строки не учтена — последняя считается тем же отсчётом,
                // и у однострочного абзаца обе совпадают).
                let mut last_shift = px(0.);
                // Верхняя надбавка ПЕРВОЙ строки опускает её базовую линию:
                // атом выше струта сдвигает текст строки вниз, и базовая
                // абзаца (для `inline-block` — его собственная, §10.8.1)
                // обязана уехать вместе с ним.
                let mut first_above = px(0.);
                let across = {
                    probe.lines = lines.clone();
                    let pads = probe.line_padding();
                    if !probe.atom_boxes.is_empty() || !probe.box_spans.is_empty() {
                        first_above = px(pads.first().map_or(0.0, |p| p.0));
                    }
                    if pads.len() > 1 {
                        let before: f32 = pads[..pads.len() - 1].iter().map(|(a, b)| a + b).sum();
                        let own = pads[pads.len() - 1].0 - pads[0].0;
                        last_shift = line_height * (pads.len() - 1) as f32 + px(before + own);
                    }
                    let extra: f32 = pads.iter().map(|(a, b)| a + b).sum();
                    (if vertical { probe.line_height } else { line_height }) * lines.len() as f32 + px(extra)
                };
                if vertical {
                    let width = if vertical_inline.is_some() { limit.unwrap_or(width) } else { width };
                    return probe.vertical_content_baselines(size(
                        known.width.unwrap_or(across), known.height.unwrap_or(width),
                    ));
                }
                let baseline = probe.measured_first_baseline(line_height, first_above, window);
                let last_baseline = baseline.map(|b| b + last_shift);
                gpui::MeasuredContent {
                    first_y: baseline,
                    last_y: last_baseline,
                    lines_y: Some(probe.content_line_baselines(baseline)),
                    ..gpui::MeasuredContent::new(size(known.width.unwrap_or(width), known.height.unwrap_or(across)))
                }
            },
        );
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<Hitbox> {
        // Предел переноса — длина строки по её физической оси.
        self.vertical_layout_origin = window.layout_origin_unrounded(*state) - window.element_offset();
        self.width_nudge = {
            let dw = window.layout_size_unrounded(*state).width - bounds.size.width;
            let one = 1.0 / window.scale_factor().max(0.01) + 1e-4;
            if self.vertical || f32::from(dw).abs() > one {
                px(0.0)
            } else {
                dw
            }
        };
        // Snapping moves an edge by at most half a device pixel; anything
        // larger means the node was placed outside its tree (`prepaint_at`).
        self.glyph_nudge = {
            let d = window.layout_origin_unrounded(*state) - bounds.origin;
            let half = 0.5 / window.scale_factor().max(0.01) + 1e-4;
            let ok = |v: Pixels| f32::from(v).abs() <= half;
            if self.vertical || !(ok(d.x) && ok(d.y)) {
                point(px(0.0), px(0.0))
            } else {
                d
            }
        };
        let limit = if self.vertical {
            bounds.size.height
        } else {
            bounds.size.width
        };
        // Коробка приходит округлённой ВНИЗ до точки устройства, а мерили её
        // по дробной ширине: строка, влезавшая ровно, на отрисовке уже не
        // влезала и рвалась заново (`hyphens-manual-011`). Возвращаем себе эту
        // одну точку устройства — иначе раскладка кадра расходится с замером.
        let scale = window.scale_factor().max(1.0);
        let limit = limit + px(1.0 / scale);
        self.indent_basis = {
            let exact = window.layout_size_unrounded(*state);
            let exact = if self.vertical { exact.height } else { exact.width };
            let snapped = if self.vertical {
                bounds.size.height
            } else {
                bounds.size.width
            };
            (f32::from(exact - snapped).abs() <= 1.0 / scale + 1e-4).then_some(exact)
        };
        self.apply_measured_fit();
        self.lines = self.split(Some(limit), window);
        self.place_atoms(*state, window, _cx);
        // Куски вне потока встают на своё место в строке: раскладываются
        // по содержимому и подготавливаются от угла своего знака.
        if !self.overlays.is_empty() {
            let segs = self.measure(window);
            let mut placed = std::mem::take(&mut self.overlays);
            let rotated = crate::interact::in_rotated_frame();
            let scale = window.scale_factor().max(0.01);
            for (at, el, how) in placed.iter_mut() {
                let next = &how.next_line;
                let origin = if *next {
                    self.next_line_point(*at, bounds)
                } else {
                    self.point_of(&segs, *at, bounds)
                };
                // Повёрнутый абзац: до-поворотная y — блочная ось экрана.
                // Коробка ложится краем туда же, куда глиф соседнего текста:
                // глиф — целая часть физической точки (`paint_glyph`), а
                // раскладка округлила бы до ближайшей. Расхождение в точку
                // оставляло столбец красного (`static-position/vlr-*`).
                let origin = if rotated {
                    let y = f32::from(origin.y + px(how.rot_dy)) * scale;
                    point(origin.x + px(how.rot_dx), px((y + 1e-3).floor() / scale))
                } else {
                    origin
                };
                // Ширина абсолютного элемента — «по содержимому» (CSS 2.1
                // §10.3.7): по МИНИМАЛЬНОМУ содержимому он рвался бы по
                // словам (`static-position/htb-*`).
                let size = el.layout_as_root(
                    gpui::size(
                        gpui::AvailableSpace::MaxContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    _cx,
                );
                // Блочная коробка при `rtl` вешается ПРАВЫМ краем на правый
                // край содержимого (§10.3.7: `right` = статическая позиция).
                let origin = if *next && self.wrap.rtl {
                    // `origin.x` здесь — край содержимого плюс сдвиг предков.
                    point(origin.x + bounds.size.width - size.width, origin.y)
                } else if how.bidi_hang && self.rtl_level_at(*at) {
                    point(origin.x - size.width, origin.y)
                } else {
                    origin
                };
                el.prepaint_at(origin, window, _cx);
            }
            self.overlays = placed;
        }
        self.id
            .is_some()
            .then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Поворачивается текст внутри уже рассчитанной по физическим осям коробки.
        if self.vertical {
            let bounds = self.vertical_paint_bounds(bounds, self.vertical_layout_origin, window.scale_factor());
            let matrix = self.vertical_transform(bounds, window.scale_factor());
            // Flat inline/block axes match the painted physical extent.
            let flat = Bounds {
                origin: bounds.origin,
                size: size(bounds.size.height, bounds.size.width),
            };
            let mut inner = std::mem::replace(self, Paragraph::empty());
            inner.vertical = false;
            let selection = inner.selection_vertical.replace((bounds, inner.vertical_ccw));
            window.with_transformation(matrix, |window| {
                inner.paint(id, _inspector_id, flat, _state, hitbox, window, cx);
            });
            inner.selection_vertical = selection;
            inner.vertical = true;
            *self = inner;
            return;
        }
        let outer_nudge = window.replace_glyph_offset(self.glyph_nudge);
        let bounds = Bounds {
            origin: bounds.origin,
            size: size(bounds.size.width + self.width_nudge, bounds.size.height),
        };
        let segs = self.measure(window);
        if self.run_metrics.len() != self.runs.len() {
            self.run_metrics = self.measure_runs(window);
        }
        let count = self.lines.len();
        // Надбавки строк от сдвинутых кусков: шаг до следующей строки и
        // сдвиг набора внутри своей.
        let pads = self.line_padding();
        let step = |i: usize| -> Pixels {
            let (a, b) = pads.get(i).copied().unwrap_or((0.0, 0.0));
            self.line_height + px(a + b)
        };
        // Надбавка сверху опускает НАБОР строки: поднятый знак занимает её,
        // а базовая линия остаётся на своём месте относительно кегля.
        let above = |i: usize| -> Pixels { px(pads.get(i).copied().unwrap_or((0.0, 0.0)).0) };
        // Строки снизу вверх: место строки считается ОТ ВЕРХА коробки одним
        // сложением (`origin + px(смещение)`), как у `point_of`. Прежде
        // `origin + total - line_height` в f32 расходился с `point_of` в
        // последнем знаке, а глиф (`paint_glyph`: `floor` физической точки)
        // на ровной точке от этого падает на целую точку: текст стоял на
        // точку от щупа статической позиции — столбец красного в
        // `static-position/vlr-*` (замер по снимку: глиф x=49, коробка 48;
        // после правки оба 48).
        let mut rev_off: f32 = if self.lines_reversed && count > 0 {
            let total: f32 = (0..count).map(|i| f32::from(step(i))).sum();
            total - f32::from(self.line_height)
        } else {
            0.0
        };
        let mut y = bounds.origin.y + px(rev_off);
        let selection = id
            .map(|global| {
                window.with_element_state::<Selection, _>(global, |state, _| {
                    let st = state.unwrap_or_default();
                    (st.range(), st)
                })
            })
            .unwrap_or((0, 0));
        let runs = self.runs_with_selection(selection.0, selection.1);
        for (i, line) in self.lines.clone().into_iter().enumerate() {
            // Перевод строки в набор не отдаём: он уже сработал разрывом.
            let body = self.text[line.range.clone()].trim_end_matches('\n');
            let range = line.range.start..line.range.start + body.len();
            // Последняя строка абзаца и строка, оборванная жёстким разрывом,
            // по ширине не растягиваются: иначе абзац из одного слова разъехался
            // бы во всю колонку. Для них своя выключка (`text-align-last`).
            // Строка с СОХРАНЁННОЙ табуляцией не растягивается: позиции
            // табуляции обязаны совпасть с нерастянутой строкой
            // (css-text-4 §8.1, `text-align-justify-tabs-001`), а раздача
            // остатка их бы сдвинула.
            let align = self.line_align(i, &line);
            // Отступ первой строки занимает место В колонке: остаток на
            // выключку считается уже без него. Правый вырез обтекания
            // (`shape-outside`) — тоже: прижатая вправо строка упирается в
            // форму, а не в край коробки (circle-024: text-align right).
            let free_raw = bounds.size.width - line.width - line.indent - px(self.flow_cut(i).1);
            // Обрезание отрицательного остатка нужно только РАЗДАЧЕ
            // (`Justify`): растягивать переполненную строку нечем. Сдвиг по
            // `text-align` берёт остаток СО ЗНАКОМ — иначе широкая строка
            // всегда вылезает вправо, то есть по-ltr при любом письме
            // (см. `line_offset`).
            let free = if free_raw < px(0.) { px(0.) } else { free_raw };
            // Свисающий открывающий знак уходит ЗА край: строка сдвигается
            // влево на его ширину. Считается до выбора пути отрисовки —
            // выключенная строка свисает так же, как обычная.
            let hang = self.hang_first(line.range.start);
            let shift = self.span(&segs, line.range.start, line.range.start + hang);
            // Отступ первой строки идёт от НАЧАЛЬНОГО края (§16.1): в rtl это
            // правый край, и место ему уже отдано вычетом из остатка выше.
            // Прибавка слева считала бы его второй раз, а при выключке вправо
            // и вовсе гасила: `(W - w - indent) + indent = W - w`.
            let lead = if self.wrap.rtl { px(0.) } else { line.indent } - shift;
            // Строка с межсловным интервалом рисуется ПО СЛОВАМ: одним
            // набором промежутки не показать — шейпер о них не знает. Раздача
            // остатка при этом нулевая, слова просто встают по своим местам.
            // Межсловный интервал ставит слова по местам сам, поэтому строка
            // с ним рисуется тем же путём, что и выключенная. Интервал бывает
            // задан и НА КУСКЕ — тогда общего значения нет, а путь нужен тот
            // же (иначе `word-spacing` на `<span>` не действовал вовсе).
            // Строка с СОХРАНЁННОЙ табуляцией рисуется тоже по словам:
            // продвижение табуляции задаёт её позиция (`Seg::offset`), а
            // сплошной набор строки о ней не знает и кладёт глиф шрифта —
            // нарисованное выходило короче замеренного на целую позицию
            // (`text-align-justify-tabs-002`). Раздача остатка при этом
            // нулевая: растягивать такую строку нельзя (см. `no_stretch`),
            // слова просто встают по своим местам.
            if align == Align::Justify
                || body.contains('\u{9}')
                || self.word_spacing != px(0.)
                || !self.word_spans.is_empty()
                || self.letter_spans_diverge()
                || !self.shift_spans.is_empty()
                || !self.rel_spans.is_empty()
                || !self.edge_spans.is_empty()
            {
                let (free, dx) = if align == Align::Justify {
                    (free, lead)
                } else {
                    let dx = line_offset(align, self.wrap.rtl, free_raw);
                    (px(0.), dx + lead)
                };
                // Набор строки опускается на её верхнюю надбавку: поднятый
                // кусок занимает добавленное место, а остальной текст
                // остаётся на своей базовой линии.
                self.paint_justified(
                    &range,
                    &segs,
                    free,
                    line.width,
                    bounds,
                    y + above(i),
                    pads.get(i).copied().unwrap_or((0.0, 0.0)),
                    dx,
                    window,
                    cx,
                );
                if line.ellipsis {
                    let text = self.span(&segs, line.range.start, range.end);
                    self.paint_suffix(
                        self.marker_str(),
                        line.range.start,
                        point(bounds.origin.x + dx + text, y + above(i)),
                        window,
                        cx,
                    );
                }
                if self.lines_reversed {
                    rev_off -= f32::from(step(i));
                    y = bounds.origin.y + px(rev_off);
                } else {
                    y += step(i);
                }
                continue;
            }
            let dx = line_offset(align, self.wrap.rtl, free_raw) + lead;
            if {
                static ON: std::sync::LazyLock<bool> =
                    std::sync::LazyLock::new(|| std::env::var("TCA_DBG").is_ok());
                *ON
            } {
                eprintln!(
                    "TCA para {:?} align={:?} bw={:?} lw={:?} free={:?}",
                    &self.text[..self.text.len().min(6)],
                    align,
                    bounds.size.width,
                    line.width,
                    free
                );
            }
            let at = point(bounds.origin.x + dx, y + above(i));
            // Висящие пробелы конца строки при письме справа налево уходят по
            // правилу L1 на ЛЕВЫЙ край и отодвигали бы текст от края коробки.
            // Рисовать их незачем: они пустые.
            // ★ ЗАМЕРЕНО: рисовать их и при `pre` — `trailing-space-and-
            // text-alignment-rtl-002` 0.02 -> 1.67 (пробел вставал справа от
            // текста и сдвигал его); место в ширине строки они держат.
            let visible = if self.wrap.rtl && !self.wrap.break_spaces {
                range.start..range.start + trim_hanging(&self.text[range.clone()])
            } else if !self.wrap.keep_spaces {
                // Схлопываемый пробел конца строки УДАЛЯЕТСЯ (CSS 2.1 §16.6.1),
                // а не висит: рисовать его незачем, а подложка `<span>` под ним
                // вылезала за край коробки квадратом кегля (`c548-leadin-000`:
                // красный 25×25 справа от первой строки). Отличие от откаченной
                // правки у `trim_hanging`: там резалась ПОДЛОЖКА прогонов по
                // всему `hangs` (U+3000, U+2000..200A и пр.), здесь — только сам
                // отрезок набора, только U+0020/U+0009 и только при схлопывающем
                // `white-space`; прочие Zs-разделители висят как прежде.
                let body = &self.text[range.clone()];
                range.start..range.start + body.trim_end_matches([' ', '\t']).len()
            } else {
                range.clone()
            };
            // Знак обрыва и знак переноса набираются вместе со строкой.
            let mark = if line.ellipsis {
                self.marker_str().to_string()
            } else if line.hyphen {
                self.hyphen.to_string()
            } else {
                String::new()
            };
            self.paint_line(&visible, &runs, at, &mark, window, cx);
            if self.lines_reversed {
                rev_off -= f32::from(step(i));
                y = bounds.origin.y + px(rev_off);
            } else {
                y += step(i);
            }
        }
        window.replace_glyph_offset(outer_nudge);
        for slot in self.atoms.iter_mut().filter(|s| !s.hidden) {
            slot.el.paint(window, cx);
        }
        for (_, el, _) in self.overlays.iter_mut() {
            el.paint(window, cx);
        }
        if let (Some(global), Some(hitbox)) = (id, hitbox.clone()) {
            self.track_selection(global, &segs, bounds, hitbox, window);
        }
    }
}

/// Память выделения между кадрами: границы в байтах текста абзаца.
#[derive(Default, Clone, Copy)]
struct Selection {
    anchor: usize,
    head: usize,
    dragging: bool,
}

impl Selection {
    fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
}

impl Paragraph {
    /// Прогоны с подложкой на выделенном куске: прогон нельзя раскрасить
    /// наполовину, поэтому попавшие на границу режутся надвое.
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

    /// Тянуть выделение мышью.
    fn track_selection(
        &self,
        global: &GlobalElementId,
        segs: &[Seg],
        bounds: Bounds<Pixels>,
        hitbox: Hitbox,
        window: &mut Window,
    ) {
        let inside = hitbox.is_hovered(window);
        if inside {
            window.set_cursor_style(gpui::CursorStyle::IBeam, &hitbox);
        }
        // Замыкания живут дольше кадра, поэтому берут СВОЙ снимок раскладки.
        let probe = Paragraph {
            plaintext: self.plaintext,
            lines_reversed: self.lines_reversed,
            flow: self.flow.clone(),
            text: self.text.clone(),
            spans: self.spans.clone(),
            word_spans: self.word_spans.clone(),
            letter_spans: self.letter_spans.clone(),
            shift_spans: self.shift_spans.clone(),
            lh_spans: self.lh_spans.clone(),
            rel_spans: self.rel_spans.clone(),
            atoms: Vec::new(),
            atom_boxes: self.atom_boxes.clone(),
            strut: self.strut,
            run_metrics: self.run_metrics.clone(),
            edge_spans: self.edge_spans.clone(),
            ruby_trim: self.ruby_trim,
            emph_spans: self.emph_spans.clone(),
            box_spans: self.box_spans.clone(),
            strut_run: None,
            strut_box: self.strut_box,
            ortho_limit: self.ortho_limit,
            runs: Vec::new(),
            font_size: self.font_size,
            line_height: self.line_height,
            align: self.align,
            align_last: self.align_last,
            ruby_justify: self.ruby_justify,
            ruby_unit: self.ruby_unit,
            letter_spacing: self.letter_spacing,
            word_spacing: self.word_spacing,
            vertical: self.vertical,
            vertical_rl: self.vertical_rl,
            vertical_central_baseline: self.vertical_central_baseline,
            vertical_ccw: self.vertical_ccw,
            selection_vertical: self.selection_vertical,
            vertical_layout_origin: self.vertical_layout_origin,
            glyph_nudge: self.glyph_nudge,
            width_nudge: self.width_nudge,
            indent_basis: self.indent_basis,
            vertical_inline: self.vertical_inline,
            hanging: self.hanging,
            indent: self.indent,
            spacers: self.spacers.clone(),
            id: None,
            highlight: self.highlight,
            wrap: self.wrap,
            lines: self.lines.clone(),
            clamp: self.clamp,
            clamp_force: self.clamp_force,
            fit_spacing_scalable: self.fit_spacing_scalable,
            fit_line_height_fixed: self.fit_line_height_fixed,
            text_overflow: false,
            overflow_marker: None,
            marker_font: None,
            marker_size: None,
            marker_color: None,
            fit: self.fit,
            tab_stop: self.tab_stop.clone(),
            hyphen: self.hyphen.clone(),
            hyphen_w: std::cell::Cell::new(self.hyphen_w.get()),
            overlays: Vec::new(),
        };
        let segs = segs.to_vec();
        window.with_element_state::<Selection, _>(global, |state, window| {
            let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));
            let index_at = {
                let probe = std::rc::Rc::new(probe);
                let segs = std::rc::Rc::new(segs);
                move |p: Point<Pixels>| probe.index_at(&segs, bounds, p)
            };

            let down = st.clone();
            let at_down = index_at.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _cx| {
                if !phase.bubble() || e.button != MouseButton::Left || !inside {
                    return;
                }
                let i = at_down(e.position);
                down.set(Selection {
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
                let i = at_move(e.position);
                if s.head != i {
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

    /// Набрать кусок строки и поставить его прогоны в ВИДИМОМ порядке.
    ///
    /// Двунаправленный текст набирается в логическом порядке, а на экран идёт
    /// в видимом. Переставлять ЗНАКИ нельзя — рвётся арабская вязь, поэтому
    /// строка режется на прогоны по уровням встроенности: порядок прогонов
    /// считаем сами, а каждый прогон набирает сам набор. Правому прогону
    /// сторона сообщается знаком управления: внутри него набор и переставит
    /// знаки, и развернёт парные скобки.
    fn paint_line(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        at: Point<Pixels>,
        suffix: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Пустой отрезок разбору двунаправленности отдавать нельзя: он берёт
        // уровень по первому знаку и падает на конце текста. Пустая строка
        // бывает у абзаца из одних пробелов и после жёсткого разрыва в конце.
        if range.start >= range.end || range.end > self.text.len() {
            // Пустой отрезок с ХВОСТОМ — это строка обрыва `line-clamp`, у
            // которой под многоточие не осталось места ни для одного слова
            // (`clamp_lines` схлопывает диапазон в `head..head`). Знак обрыва
            // рисуется в цикле по прогонам ниже, поэтому ранний выход уносил
            // и его: коробка занимала высоту, но многоточия не показывала
            // (`text-wrap-balance-line-clamp-004`).
            if !suffix.is_empty() && !self.text.is_empty() {
                let anchor = range.start.min(self.text.len().saturating_sub(1));
                self.paint_suffix(suffix, anchor, at, window, cx);
            }
            return;
        }
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        // `unicode-bidi: plaintext`: базу КАЖДОГО абзаца (между жёсткими
        // разрывами) выбирает первый сильный знак (UAX9 P2/P3) — разбор без
        // навязанного уровня делает ровно это. Выключка уже решается так же
        // построчно (см. own_align выше).
        let forced = if self.plaintext.is_some() {
            None
        } else {
            Some(base)
        };
        let info = unicode_bidi::BidiInfo::new(&self.text, forced);
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= range.start && range.start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return;
        };
        let (levels, visual) = info.visual_runs(para, range.clone());
        let mut x = at.x;
        for run in visual.into_iter() {
            let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
            // Знак обрыва — у обрезанного КРАЯ: обычно это логический
            // конец строки; при письме справа налево контейнер режет левый
            // край, то есть логическое НАЧАЛО — знак идёт префиксом
            // первого прогона.
            // Строка-замена (`text-overflow: "…"`) рисуется ОТДЕЛЬНЫМ
            // набором — как и при письме справа налево, и как это уже
            // делает `paint_justified`. Вплетение её в набор последнего
            // прогона (`shape` приклеивает суффикс к последнему куску)
            // отдавало ей шрифт И кегль обрезанного куска: эмодзи-маркер в
            // Ahem не рисовался вовсе (`text-overflow-string-003`), а
            // «你好 🟢» выходил кеглем 30px (`-013`). Многоточие и знак
            // переноса остаются вплетёнными: они обязаны сесть на базовую
            // линию строки (`hyphens-manual-011`).
            let own_mark = !suffix.is_empty() && self.overflow_marker.as_deref() == Some(suffix);
            let (tail, at_start) = if self.wrap.rtl {
                (if run.start == range.start { suffix } else { "" }, true)
            } else if own_mark {
                ("", false)
            } else {
                (if run.end == range.end { suffix } else { "" }, false)
            };
            let Some(shaped) = self.shape_with_mark(&run, runs, rtl, tail, at_start, window) else {
                continue;
            };
            let width = shaped.width;
            if at_start && !tail.is_empty() {
                // Знак обрыва СЛЕВА от куска: рисуется на своём месте, а
                // кусок сдвигается на его ширину.
                let ell = self.suffix_width(tail, run.start, window);
                self.paint_suffix(tail, run.start, point(x, at.y), window, cx);
                x += ell;
            }
            // Подложка прогона (`background` на `<span>`) рисуется ОТДЕЛЬНЫМ
            // вызовом: `paint` кладёт только глифы. Пока его не звали, фон
            // строчного элемента не появлялся вовсе — проверено пробой, где
            // `background: green; color: transparent` давал пустую страницу.
            let _ = shaped.paint_background(point(x, at.y), self.line_height, window, cx);
            let origin = self.text_raster_origin(&shaped, point(x, at.y), window);
            let _ = shaped.paint(origin, self.line_height, window, cx);
            x += width;
        }
        // Строка-замена — за текстом строки, своим шрифтом и кеглем.
        if !self.wrap.rtl && !suffix.is_empty() && self.overflow_marker.as_deref() == Some(suffix) {
            let anchor = range.end.saturating_sub(1).max(range.start);
            self.paint_suffix(suffix, anchor, point(x, at.y), window, cx);
        }
    }

    /// Многоточие обрыва: набирается стилем того куска, на котором строка
    /// оборвана, и рисуется сразу за её текстом.
    fn paint_suffix(
        &self,
        mark: &str,
        at: usize,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut runs = slice_runs(&self.runs, &(at..at + 1));
        let Some(run) = runs.first_mut() else {
            return;
        };
        run.len = mark.len();
        self.style_marker_run(mark, run);
        let piece = vec![run.clone()];
        let shaped = window.text_system().shape_line_spaced(
            SharedString::from(mark.to_string()),
            self.font_size,
            &piece,
            None,
            self.letter_spacing,
        );
        let origin = self.text_raster_origin(&shaped, origin, window);
        let _ = shaped.paint(origin, self.line_height, window, cx);
    }

    /// Строковый маркер несёт шрифт контейнера; многоточие и знак переноса
    /// остаются в шрифте прогона у среза.
    fn style_marker_run(&self, mark: &str, run: &mut TextRun) {
        // Знак обрыва — содержимое САМОГО БЛОКА, а не куска, на котором
        // строка оборвалась: кегль у него блочный (css-overflow-3 §4.1,
        // «the ellipsis is styled as the block»). Прогон брался с места
        // обрыва вместе со своим кеглем, и внутри `<span style="font-size:
        // 1rem">` в блоке с `4rem` многоточие выходило вчетверо уже нужного
        // (`text-wrap-balance-line-clamp-002`: место под него при подборе
        // колонки считалось 8.8 точки вместо 35.2).
        run.font_size = None;
        // Знак обрыва — анонимный строчный ребёнок САМОГО БЛОКА
        // (css-overflow-4 §5.3 block-ellipsis: «wrapped in an anonymous
        // inline whose parent is the block container»): шрифт, кегль и цвет —
        // блочные, рамки и фона куска у среза у него нет. Эталоны:
        // `block-ellipsis-005` (знак за `<span>` 1.5em bold italic — обычный
        // teal блока), `webkit-line-clamp-031` (за жирным — нежирный).
        if mark == ELLIPSIS
            && self.overflow_marker.is_none()
            && let Some(f) = self.marker_font.as_ref()
        {
            run.font = f.clone();
            run.font_size = self.marker_size;
            if let Some(c) = self.marker_color {
                run.color = c;
            }
            run.background_color = None;
            run.background_border = None;
            run.background_pad = Default::default();
            run.background_radius = px(0.);
            return;
        }
        if let (Some(m), Some(f)) = (self.overflow_marker.as_deref(), self.marker_font.as_ref())
            && mark == m
        {
            run.font = f.clone();
            // Кегль СТРОКИ-ЗАМЕНЫ — блочный, а не базовый кегль абзаца:
            // базовый равен `biggest`, и внутри `<span>` крупнее блока
            // маркер выходил втрое шире (`text-overflow-string-*`: под
            // строку резервировалось ~90 точек вместо 20).
            run.font_size = self.marker_size;
        }
    }

    /// Набор с знаком обрыва в начале или в конце куска.
    fn shape_with_mark(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        rtl: bool,
        suffix: &str,
        at_start: bool,
        window: &mut Window,
    ) -> Option<gpui::ShapedLine> {
        if !at_start || suffix.is_empty() {
            return self.shape(range, runs, rtl, suffix, window);
        }
        // Префикс: знак дорисовывается отдельным вызовом слева, а сам кусок
        // набирается без него (вплетение в шейп меняло бы кернинг начала).
        self.shape(range, runs, rtl, "", window)
    }

    /// Набрать кусок текста; правый прогон — со знаком стороны письма.
    fn shape(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        rtl: bool,
        suffix: &str,
        window: &mut Window,
    ) -> Option<gpui::ShapedLine> {
        let mut piece = slice_runs(runs, range);
        if piece.is_empty() {
            return None;
        }
        // Управляющие знаки не рисуются: своей ширины у них нет, но подмена
        // шрифта может подставить вместо них пустой квадрат и раздвинуть
        // строку. Разрывы по ним УЖЕ решены — здесь остаётся только показ.
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: не выбрасывать их, чтобы замер и показ считали
        // один и тот же текст (замер берёт его целиком). Счёт не изменился,
        // а `trim_runs` и `invisible` становились мёртвым кодом.
        let body: String = self.text[range.clone()]
            .chars()
            .filter(|c| !invisible(*c))
            .collect();
        if body.len() != range.len() {
            piece = trim_runs(&piece, &self.text[range.clone()]);
        }
        // Знак переноса набирается ВМЕСТЕ со строкой, а не отдельным вызовом:
        // отдельный набор садится на свою базовую линию и сдвигает строку
        // (`hyphens-manual-011`: текст уезжал на три точки вниз).
        let body = if suffix.is_empty() {
            body
        } else {
            // Знак обрыва — свой прогон в стиле блока; знак переноса —
            // часть слова и идёт стилем своего куска.
            if suffix == ELLIPSIS && self.overflow_marker.is_none() && self.marker_font.is_some() {
                if let Some(last) = piece.last() {
                    let mut run = last.clone();
                    run.len = suffix.len();
                    self.style_marker_run(suffix, &mut run);
                    piece.push(run);
                }
            } else if let Some(last) = piece.last_mut() {
                last.len += suffix.len();
            }
            format!("{body}{suffix}")
        };
        let body = body.as_str();
        let body = controlled_shape::text(body, &mut piece, rtl);
        if !rtl {
            return Some(window.text_system().shape_line_spaced(
                body,
                self.font_size,
                &piece,
                None,
                self.letter_spacing,
            ));
        }
        Some(window.text_system().shape_line_rtl(
            body,
            self.font_size,
            &piece,
            self.letter_spacing,
        ))
    }

    /// Выключка по ширине: остаток строки раздаётся её пробелам.
    ///
    /// Слова набираются по отдельности и ставятся каждое на своё место —
    /// иначе растянуть промежутки нечем: набор отдаёт готовую строку одним
    /// куском. Внутри слова набор остаётся сплошным, поэтому лигатуры и вязь
    /// не рвутся.
    #[allow(clippy::too_many_arguments)]
    fn paint_justified(
        &self,
        range: &std::ops::Range<usize>,
        segs: &[Seg],
        free: Pixels,
        line_width: Pixels,
        bounds: Bounds<Pixels>,
        y: Pixels,
        pad: (f32, f32),
        dx: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut words = self.words(range);
        // Слово режется по границам кусков с трекингом: набор принимает
        // трекинг скаляром, поэтому кусок с другим значением обязан идти
        // отдельным вызовом. Без этого `letter-spacing` на `<span>` внутри
        // слова не действовал вовсе.
        if !self.letter_spans.is_empty()
            || !self.shift_spans.is_empty()
            || !self.rel_spans.is_empty()
            || !self.edge_spans.is_empty()
        {
            let mut cuts: Vec<usize> = Vec::new();
            let mut cut = |edge: usize| {
                if edge > range.start && edge < range.end {
                    cuts.push(edge);
                }
            };
            for (r, _) in self.letter_spans.iter() {
                // Трекинг знака — это промежуток ПОСЛЕ него, поэтому у
                // последнего знака отрезка он на набор внутри отрезка не
                // влияет. Резать по началу нужно только там, где знаков в
                // диапазоне несколько: одиночный (зазор `text-autospace`)
                // спокойно доживает в общем отрезке, а свой разрез оставлял
                // между половинками слова шов в точку — соседние отрезки
                // округляются независимо (`text-autospace-001`: `XX`
                // расходились).
                // A box spacer carries its entire advance in tracking (CSS 2.1
                // section 8.3). Joining it to a preceding word discards that
                // advance when the word is shaped with its own spacing.
                if self.text[r.clone()].chars().nth(1).is_some()
                    || self.spacers.binary_search(&r.start).is_ok()
                {
                    cut(r.start);
                }
                cut(r.end);
            }
            // Сдвиг по вертикали — свойство самого глифа: он обязан ехать
            // отдельным вызовом целиком.
            for (r, _) in self.rel_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            for (r, _) in self.shift_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            for (r, _, _) in self.edge_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            cuts.sort_unstable();
            cuts.dedup();
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let mut spaces = w.spaces_before;
                for cut in cuts.iter().copied().filter(|c| w.range.contains(c)) {
                    split.push(Word {
                        range: at..cut,
                        spaces_before: spaces,
                    });
                    at = cut;
                    spaces = 0;
                }
                split.push(Word {
                    range: at..w.range.end,
                    spaces_before: spaces,
                });
            }
            words = split;
        }
        // Растягивается КАЖДЫЙ пробел, а не промежуток между словами: там, где
        // подряд стоят два сохранённых пробела, добавка идёт дважды.
        //
        // Пробелы ЛЕВЕЕ последней табуляции добавки не получают: табуляция
        // доводит строку до своей позиции и всё лишнее место слева от себя
        // поглощает, поэтому позиции табуляции совпадают с нерастянутой
        // строкой (css-text-4 §8.1). Отсюда оба поведения сразу: строка, где
        // все пробелы левее табуляции, не растягивается вовсе
        // (`text-align-justify-tabs-001`, обе коробки обязаны совпасть), а
        // остаток достаётся только пробелам правее (`-002`: их ровно два, и
        // каждый вырастает на пробел).
        let absorbed = self.text[range.clone()].rfind('\u{9}').map_or(0, |at| {
            self.text[range.start..range.start + at]
                .chars()
                .filter(|c| word_separator(*c))
                .count()
        });
        if absorbed > 0 {
            for w in words.iter_mut() {
                w.spaces_before = w.spaces_before.saturating_sub(absorbed);
            }
        }
        let opportunities = words.last().map(|w| w.spaces_before).unwrap_or(0);
        let step = if opportunities > 0 {
            free / opportunities as f32
        } else {
            px(0.)
        };
        let from = self.x_at(segs, range.start, Edge::Start);
        // Ось зеркала правой строки — её СОБСТВЕННЫЙ правый край, а не край
        // коробки: прижим уже учтён в `dx`, и вычитать его из ширины коробки
        // значит ошибиться на `free − 2·dx` (`bidi-box-model-013`: 380 точек).
        // ★ ЗАМЕРЕНО, ЭФФЕКТА НЕТ (01.09): зажимать ось шириной коробки
        // (`line_width.min(bounds.size.width)`), чтобы переполняющая строка в
        // rtl росла ВЛЕВО, как это делают блоки. Срез из 157 пар rtl/bidi —
        // 139 → 139, `absolute-non-replaced-width-021/022/023` остались 0.85.
        // Зажим не срабатывает: `bounds` здесь — коробка САМОГО абзаца, и она
        // уже растянута по содержимому. Корень выше: абзац не зажат
        // `max-width` родителя, а не ось зеркала.
        let mirror = bounds.origin.x + dx + line_width + free;
        // UAX#9 L2 разворачивает прогоны уровня ≥1, а зеркало строки
        // переворачивает ВСЕ слова разом: латинский прогон внутри правого
        // абзаца выходил задом наперёд. Прогоны левого уровня выкладываем
        // заново — в своём порядке и на своём месте (выключенный путь
        // `BidiInfo` не считал вовсе).
        //
        // ЗАМЕРЕНО: приобретено 1, потерь нет. Ожидалось больше: в семье
        // `bidi-box-model-*` левый прогон чаще всего ОДНО слово, а одиночное
        // слово зеркало кладёт верно и без разбора. Остаток той семьи держат
        // распорки полей, а не порядок слов.
        // ПРОБОВАЛИ И ОТКАТИЛИ: распространить разворот и на ЛЕВЫЕ строки
        // (правый прогон внутри левого абзаца). Замерено: 5063 -> 5059, четыре
        // пары `CSS2/bidi-005..009` перешли порог 0.50 -> 0.51. Прогоны там
        // однознаковые, и разворот нужен ПОГЛИФНЫЙ, а наш идёт по словам.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.10): поглифный вариант — прогоны UAX#9 L2
        // левой строки в видимом порядке, слово правого уровня набором
        // `shape_line_rtl`, в правой строке слово правого уровня тоже
        // справа налево и место без трекинга хвоста, плюс накопительный
        // счёт пробелов у разрезанного трекингом слова. Тестовые половины
        // `letter-spacing-bidi-003` встали верно, но эталон держит флоат,
        // уходящий строкой ниже, — пара красная. Срез 845 пар (bidi,
        // letter-spacing, text-justify, text-align, word-spacing, CSS2/text):
        // 779 -> 779, `bidi-011` 1.05 -> 2.26 (распорки полей `<span>` с RLO
        // остаются на логическом месте). Возвращать вместе с распорками,
        // переставляемыми по прогонам (патч: `target/perword-bidi-2026-10-04.patch`).
        let mut logical_run: Vec<(usize, Pixels)> = vec![];
        if self.wrap.rtl && range.start < range.end && range.end <= self.text.len() {
            let info = unicode_bidi::BidiInfo::new(&self.text, Some(unicode_bidi::Level::rtl()));
            if let Some(para) = info
                .paragraphs
                .iter()
                .find(|p| p.range.start <= range.start && range.start < p.range.end)
                .or_else(|| info.paragraphs.first())
            {
                let (levels, visual) = info.visual_runs(para, range.clone());
                for run in visual {
                    if levels.get(run.start).is_some_and(|l| l.is_rtl()) {
                        continue;
                    }
                    let idx: Vec<usize> = words
                        .iter()
                        .enumerate()
                        .filter(|(_, w)| w.range.start >= run.start && w.range.start < run.end)
                        .map(|(i, _)| i)
                        .collect();
                    if idx.len() < 2 {
                        continue;
                    }
                    // Зеркальные места прогона: слева лежит ПОСЛЕДНЕЕ слово.
                    let mut mirrored: Vec<(usize, Pixels, Pixels)> = idx
                        .iter()
                        .map(|&i| {
                            let w = &words[i];
                            let width = self.word_width(w, window);
                            let logical = (self.x_at(segs, w.range.start, Edge::Start) - from)
                                + step * w.spaces_before as f32;
                            let x = mirror - logical - width;
                            (i, x, width)
                        })
                        .collect();
                    mirrored
                        .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    // Промежутки между зеркальными соседями: в логическом
                    // порядке те же самые, только в обратную сторону.
                    let gaps: Vec<Pixels> = mirrored
                        .windows(2)
                        .map(|p| p[1].1 - (p[0].1 + p[0].2))
                        .rev()
                        .collect();
                    let mut cursor = mirrored[0].1;
                    for (k, &i) in idx.iter().enumerate() {
                        let width = mirrored
                            .iter()
                            .find(|(j, _, _)| *j == i)
                            .map(|(_, _, w)| *w)
                            .unwrap_or(px(0.));
                        logical_run.push((i, cursor));
                        cursor += width + gaps.get(k).copied().unwrap_or(px(0.));
                    }
                }
            }
        }
        let line_base = self.base_of(range);
        for (wi, word) in words.iter().enumerate() {
            let slice: SharedString = self.text[word.range.clone()].to_string().into();
            // Полоса строчной коробки продолжается сквозь слова (см.
            // `slice_runs_banded`); при rtl слова зеркалятся, и стороны
            // меняются местами — там прежний счёт.
            let runs = if self.wrap.rtl {
                slice_runs(&self.runs, &word.range)
            } else {
                slice_runs_banded(&self.runs, &word.range)
            };
            let shaped = window.text_system().shape_line_spaced(
                slice,
                self.font_size,
                &runs,
                None,
                self.letter_spans
                    .iter()
                    .find(|(r, _)| r.contains(&word.range.start))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.letter_spacing),
            );
            let logical = (self.x_at(segs, word.range.start, Edge::Start) - from)
                + step * word.spaces_before as f32;
            // При письме справа налево строка раздаётся от ПРАВОГО края:
            // первое слово встаёт справа, последнее — слева. Раздача слева
            // направо переворачивала порядок слов на выключенной строке.
            let x = match logical_run.iter().find(|(i, _)| *i == wi) {
                Some((_, fixed)) => *fixed,
                None if self.wrap.rtl => mirror - logical - shaped.width,
                None => bounds.origin.x + dx + logical,
            };
            // Сдвиг куска по вертикали: надстрочный и подстрочный знак стоят
            // выше и ниже базовой линии, оставаясь в той же строке.
            let dy = self
                .shift_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or(px(0.));
            // Относительный сдвиг двигает ТОЛЬКО отрисовку куска: место в
            // потоке за ним сохраняется, соседи не съезжают (§9.4.3).
            let (rx, ry) = self
                .rel_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or((0.0, 0.0));
            // Слово набирается своим вызовом, и `ShapedLine::paint` ставит
            // его базовую линию по СВОИМ подъёму и спуску. Сплошной набор
            // строки берёт наибольшие по всей строке — куски разного кегля
            // стоят на одной базовой линии (§10.8). Слово опускается на
            // разницу: у строки из одного шрифта она ровно ноль.
            let fix = match (line_base, self.base_of(&word.range)) {
                (Some(l), Some(w)) => px(l - w),
                _ => px(0.),
            };
            // Кусок у края строки: его строчная коробка (высотой своей
            // `line-height`, глифы по её полулидингу) встаёт верхом на верх
            // строки или низом на низ (§10.8.1). `shaped.paint` центрирует
            // глифы в высоте строки абзаца — разница высот делится пополам.
            let fix = match self
                .edge_spans
                .iter()
                .find(|(r, _, _)| r.contains(&word.range.start))
            {
                Some((_, top, h)) => {
                    let lh = f32::from(self.line_height);
                    let line_top = -pad.0;
                    let box_top = if *top { line_top } else { lh + pad.1 - h };
                    px(box_top + (h - lh) / 2.0)
                }
                None => fix,
            };
            let at = point(x + px(rx), y + dy + px(ry) + fix);
            // Подложка прогона — отдельным вызовом, см. выше.
            let _ = shaped.paint_background(at, self.line_height, window, cx);
            let origin = self.text_raster_origin(&shaped, at, window);
            let _ = shaped.paint(origin, self.line_height, window, cx);
            // Пробелы между словами тоже принадлежат полосе коробки: без
            // этого фон и рамка `<span>` рвались на каждом пробеле. Промежуток
            // набирается своими прогонами (обе стороны — продолжение полосы) и
            // красит только подложку. Растянутые выключкой промежутки красит
            // ветка ниже.
            if step == px(0.)
                && !self.wrap.rtl
                && let Some(next) = words.get(wi + 1)
                && next.range.start > word.range.end
            {
                let gap = word.range.end..next.range.start;
                let gap_runs = slice_runs_banded(&self.runs, &gap);
                if gap_runs.iter().any(|r| r.background_color.is_some()) {
                    let gap_text: SharedString = self.text[gap.clone()].to_string().into();
                    let spacing = self
                        .letter_spans
                        .iter()
                        .find(|(r, _)| r.contains(&gap.start))
                        .map(|(_, v)| *v)
                        .unwrap_or(self.letter_spacing);
                    let mut gap_shaped = window.text_system().shape_line_spaced(
                        gap_text.clone(),
                        self.font_size,
                        &gap_runs,
                        None,
                        spacing,
                    );
                    // Ширина промежутка — по РАЗЛОЖЕННОЙ строке: там в нём уже
                    // лежит `word-spacing`, а отдельный набор пробела его не
                    // знает, и полоса `<span>` рвалась на каждом растянутом
                    // пробеле (`word-spacing-characters-001`). Недостача
                    // раздаётся трекингом по знакам промежутка.
                    let want = self.x_at(segs, gap.end, Edge::Start)
                        - self.x_at(segs, gap.start, Edge::Start);
                    let n = gap_text.chars().count().max(1) as f32;
                    if (want - gap_shaped.width).abs() > px(0.5) {
                        gap_shaped = window.text_system().shape_line_spaced(
                            gap_text,
                            self.font_size,
                            &gap_runs,
                            None,
                            spacing + (want - gap_shaped.width) / n,
                        );
                    }
                    let gap_x =
                        bounds.origin.x + dx + (self.x_at(segs, gap.start, Edge::Start) - from);
                    let gap_y = match (line_base, self.base_of(&gap)) {
                        (Some(l), Some(w)) => y + px(l - w),
                        _ => y,
                    };
                    let _ = gap_shaped.paint_background(
                        point(gap_x, gap_y),
                        self.line_height,
                        window,
                        cx,
                    );
                }
            }
            // Растянутый выключкой пробел тоже принадлежит прогону, и его
            // подложка обязана быть сплошной. Красим ТОЛЬКО когда пробел
            // целиком внутри одного прогона с фоном — иначе фон соседнего
            // куска растекается по чужому месту (замерено: css-text 1015 →
            // 691 при покраске каждого промежутка).
            if step > px(0.)
                && !self.wrap.rtl
                && let Some(next) = words.get(wi + 1)
                && next.range.start > word.range.end
                && let Some(bg) = self.gap_background(word, next)
            {
                let after = (self.x_at(segs, next.range.start, Edge::Start) - from)
                    + step * next.spaces_before as f32;
                let right = bounds.origin.x + dx + after;
                let left = x + shaped.width;
                if right > left {
                    window.paint_quad(gpui::fill(
                        Bounds {
                            origin: point(left, y + dy),
                            size: gpui::size(right - left, self.line_height),
                        },
                        bg,
                    ));
                }
            }
        }
    }

    /// Ширина слова в наборе — тем же путём, что и отрисовка.
    /// ПРОБОВАЛИ И ОТКАТИЛИ: чистить слово от незримых знаков перед набором
    /// и замером, как это делает общий путь (`trim_runs`). Проба по 296 парам
    /// семей `bidi-*`, `letter-spacing-*`, `shaping-arabic-*`: не сдвинулась
    /// НИ ОДНА — знаки управления двунаправленностью лежат в своих кусках, а
    /// не внутри слов, и в этот путь не попадают.
    fn word_width(&self, word: &Word, window: &mut Window) -> Pixels {
        let slice: SharedString = self.text[word.range.clone()].to_string().into();
        let runs = slice_runs(&self.runs, &word.range);
        window
            .text_system()
            .shape_line_spaced(
                slice,
                self.font_size,
                &runs,
                None,
                self.letter_spans
                    .iter()
                    .find(|(r, _)| r.contains(&word.range.start))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.letter_spacing),
            )
            .width
    }

    /// Подложка промежутка между словами — только если оба соседа и сам
    /// промежуток лежат в ОДНОМ прогоне, и у него есть фон.
    fn gap_background(&self, left: &Word, right: &Word) -> Option<gpui::Hsla> {
        let run_at = |at: usize| -> Option<usize> {
            let mut start = 0usize;
            for (i, run) in self.runs.iter().enumerate() {
                if at < start + run.len {
                    return Some(i);
                }
                start += run.len;
            }
            None
        };
        let a = run_at(left.range.end.saturating_sub(1))?;
        let b = run_at(right.range.start)?;
        let gap = run_at(left.range.end)?;
        if a != b || a != gap {
            return None;
        }
        self.runs[a].background_color
    }

    /// Слова строки — куски между пробелами, каждое со счётом пробелов слева.
    fn words(&self, range: &std::ops::Range<usize>) -> Vec<Word> {
        let mut out: Vec<Word> = Vec::new();
        let mut start = None;
        let mut spaces = 0usize;
        for (i, ch) in self.text[range.clone()].char_indices() {
            let at = range.start + i;
            // Разделитель слов для выключки — не любой пробел. По css-text-3
            // это пробел, неразрывный и идеографический; ТАБУЛЯЦИЯ в него не
            // входит: она доводит строку до своей позиции, и растягивать её
            // нечем. Пока табуляция раскрывалась в пробелы и каждый считался
            // точкой раздачи, остаток размазывался по ней вместо слов.
            if word_separator(ch) {
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
                spaces += usize::from(!self.ruby_justify);
            } else if ch == '\u{9}' {
                // Табуляция — ГРАНИЦА слова, хотя точкой раздачи и не служит.
                // Её продвижение задаёт позиция табуляции (`Seg::offset`), и
                // внутри слова оно пропадало: строка без пробелов уходила в
                // набор одним куском, и табуляция рисовалась глифом шрифта
                // (`text-indent-tab-positions-001`: `a⇥b⇥c` выходило `abc`).
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
            } else if start.is_none() {
                start = Some(at);
            }
        }
        if let Some(s) = start {
            out.push(Word {
                range: s..range.end,
                spaces_before: spaces,
            });
        }
        out
    }
}

/// Куски оформления, попавшие в отрезок строки.
/// Прогоны отрезка для ПОСЛОВНОЙ отрисовки полосы строчной коробки.
///
/// Слово, вырезанное из середины `<span>` с фоном или рамкой, — не начало и не
/// конец коробки: полоса продолжается в соседние знаки той же коробки, и поле
/// с боковой гранью на этой стороне не ставится (css-break-3
/// `box-decoration-break: slice`; на переносе то же делает сплошной набор,
/// `vendor/gpui` `line.rs` `run_background_quad` `pad_left/pad_right`).
/// Прежде каждое слово рисовало полную коробку — с рамкой и полем с обеих
/// сторон, и `<span>` с рамкой распадался на коробки по словам.
///
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.10): полоса для `<span>` с рамкой БЕЗ фона —
/// прозрачная подложка прогона плюс `inline_pad` в `inline.rs`, чтобы
/// `run_background_quad` рисовал и такую рамку. Даже с отсечкой rtl,
/// `unicode-bidi: bidi-override`, сильных R/AL и знаков направления срез 3000
/// пар дал +1/−14, срез 125 строчных пар +7/−16: теряет семья `bidi-*` —
/// пословная отрисовка ltr-абзаца со знаками RLO/LRO идёт в ЛОГИЧЕСКОМ
/// порядке, и полоса на каждый видимый прогон рисует боковые грани дважды.
/// Возвращаться вместе с двунаправленной раскладкой полос (box-decoration по
/// видимым фрагментам, css-break-3 §5.4).
fn slice_runs_banded(runs: &[TextRun], range: &std::ops::Range<usize>) -> Vec<TextRun> {
    let mut out = slice_runs(runs, range);
    let band_at = |at: usize| -> Option<(Option<Hsla>, Option<(Hsla, [Pixels; 4])>)> {
        let mut start = 0usize;
        for run in runs {
            if at < start + run.len {
                return Some((run.background_color, run.background_border));
            }
            start += run.len;
        }
        None
    };
    if range.start > 0
        && let Some(first) = out.first_mut()
        && first.background_color.is_some()
        && band_at(range.start - 1) == Some((first.background_color, first.background_border))
    {
        first.background_pad[3] = px(0.);
        if let Some(b) = first.background_border.as_mut() {
            b.1[3] = px(0.);
        }
    }
    if let Some(last) = out.last_mut()
        && last.background_color.is_some()
        && band_at(range.end) == Some((last.background_color, last.background_border))
    {
        last.background_pad[1] = px(0.);
        if let Some(b) = last.background_border.as_mut() {
            b.1[1] = px(0.);
        }
    }
    out
}

fn slice_runs(runs: &[TextRun], range: &std::ops::Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for run in runs {
        let end = at + run.len;
        let from = at.max(range.start);
        let to = end.min(range.end);
        if from < to {
            let mut piece = run.clone();
            piece.len = to - from;
            out.push(piece);
        }
        at = end;
        if at >= range.end {
            break;
        }
    }
    out
}

impl IntoElement for Paragraph {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Правила переноса из стиля — и признак, нужна ли своя раскладка вовсе.
///
/// Пока своя раскладка не умеет выделение мышью, поэтому обычный текст
/// остаётся на выделяемом элементе движка. Сюда уходит только то, что иначе
/// не выразить.
/// Правила переноса из стиля.
///
/// Своя раскладка считает ВЕСЬ текст: перенос, выключка и свисающая
/// пунктуация должны решаться одним алгоритмом, иначе соседние абзацы одной
/// страницы ломаются по-разному. Поэтому правила есть всегда — отбор «кому
/// своя раскладка нужна, а кому нет» отсюда снят.
pub fn rules(c: &crate::computed::Computed) -> Option<Wrap> {
    Some(wrap_of(c))
}

/// Сдвиг строки вдоль коробки по `text-align` — с УЧЁТОМ ЗНАКА остатка.
///
/// Дословный перенос Blink `length_utils.cc:1607 LineOffsetForTextAlign`.
/// Смысл в том, что обрезание отрицательного остатка зависит от СТОРОНЫ
/// ПИСЬМА БЛОКА, а не от значения `text-align`:
///
/// * ltr — отрицательный остаток гасится всегда: «Wide lines spill out of the
///   block based off direction. So even if text-align is right, if direction
///   is LTR, wide lines should overflow out of the right side of the block»
///   (`length_utils.cc:1634-1636`);
/// * rtl — не гасится никогда: «The direction of the block should determine
///   what happens with wide lines. In particular with RTL blocks, wide lines
///   should still spill out to the left» (`length_utils.cc:1620-1622`).
///
/// По спеке это css-text-4 §7.1 (`right` — «Inline-level content is aligned to
/// the line-right edge of the line box», без оговорки на переполнение) вместе
/// с CSS 2.1 §16.2 (начальное значение `text-align` в rtl действует как
/// `right`) и §9.4.2 («then the inline box overflows the line box»).
///
/// `Justify` сюда не заходит: раздача остатка идёт своим путём и берёт
/// остаток УЖЕ обрезанным — растягивать переполненную строку нечем.
fn line_offset(align: Align, rtl: bool, free: Pixels) -> Pixels {
    let zero = px(0.);
    match align {
        Align::Right if rtl => free,
        Align::Right => free.max(zero),
        Align::Left if rtl => free.min(zero),
        Align::Left => zero,
        // При rtl и положительном остатке — та же половина, что и при ltr;
        // при отрицательном строка держится правого края целиком.
        Align::Center if rtl && free <= zero => free,
        Align::Center => (free / 2.).max(zero),
        Align::Justify => zero,
    }
}

/// Выключка из стиля.
pub fn align_of(a: Option<crate::computed::TextAlign>) -> Align {
    a.map(align_of_value).unwrap_or(Align::Left)
}

/// Выключка абзаца с разворотом логических краёв по стороне письма.
pub fn align_for(c: &crate::computed::Computed) -> Align {
    let rtl = c.rtl == Some(true);
    if {
        static ON: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| std::env::var("TA_DBG").is_ok());
        *ON
    } {
        eprintln!("TA align_for rtl={rtl} ta={:?}", c.text_align);
    }
    let value = c
        .text_align
        .unwrap_or(crate::computed::TextAlign::Start)
        .physical(rtl);
    let align = align_of_value(value);
    // `text-justify: none` — растягивать запрещено, и строка идёт к началу:
    // у письма справа налево началом служит правый край.
    if align == Align::Justify && c.no_justify == Some(true) {
        return if rtl { Align::Right } else { Align::Left };
    }
    align
}

/// Выключка из заданного значения.
pub fn align_of_value(a: crate::computed::TextAlign) -> Align {
    match a {
        crate::computed::TextAlign::Center => Align::Center,
        crate::computed::TextAlign::Right => Align::Right,
        crate::computed::TextAlign::Justify => Align::Justify,
        _ => Align::Left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Абзац без набора: точки разрыва считаются по тексту и правилам, а
    /// система шрифтов для этого не нужна.
    fn para(text: &str, wrap: Wrap) -> Paragraph {
        Paragraph::new(
            SharedString::from(text.to_string()),
            vec![],
            px(16.),
            px(16.),
            Align::Left,
            wrap,
        )
    }

    fn stops(text: &str, wrap: Wrap) -> Vec<usize> {
        para(text, wrap)
            .opportunities()
            .iter()
            .map(|s| s.at)
            .collect()
    }

    #[test]
    fn a_hard_break_is_a_stop_even_without_wrapping() {
        let wrap = Wrap {
            nowrap: true,
            ..Default::default()
        };
        assert_eq!(stops("a\nb", wrap), vec![2]);
    }

    #[test]
    fn break_spaces_stops_after_every_kept_space() {
        let wrap = Wrap {
            break_spaces: true,
            ..Default::default()
        };
        // Пробел даёт точку разрыва ПОСЛЕ себя — и первый, и второй.
        assert_eq!(stops("a  b", wrap), vec![2, 3]);
    }

    #[test]
    fn break_all_stops_between_letters_but_not_before_a_space() {
        let wrap = Wrap {
            break_all: true,
            ..Default::default()
        };
        // Между буквами — точка, перед пробелом — нет: рвать там нечего,
        // пробел и так свисает за край. После пробела точка от типографики.
        assert_eq!(stops("ab c", wrap), vec![1, 3]);
    }

    #[test]
    fn anywhere_stops_before_every_character() {
        let wrap = Wrap {
            anywhere: true,
            ..Default::default()
        };
        assert_eq!(stops("a b", wrap), vec![1, 2]);
    }

    #[test]
    fn keep_all_keeps_ideographs_together() {
        let wrap = Wrap {
            keep_all: true,
            ..Default::default()
        };
        // Между иероглифами разрыва нет, около пробела — есть.
        assert_eq!(stops("中文 中文", wrap), vec![7]);
    }

    #[test]
    fn hanging_spaces_are_cut_off_the_measured_part() {
        assert_eq!(trim_hanging("ab  "), 2);
        assert_eq!(trim_hanging("ab"), 2);
        assert_eq!(trim_hanging("  "), 0);
    }

    /// Распорка строчной коробки не должна съедать точку переноса ПЕРЕД собой:
    /// её класс по UAX-14 (WJ) запрещает разрыв с обеих сторон, и пробел
    /// перед `<span>` с отступом переставал быть точкой переноса.
    #[test]
    fn spacer_keeps_the_break_before_it() {
        let text = "aaa \u{feff}bbb";
        let mut para = para(text, Wrap::default());
        assert!(
            !para.opportunities().iter().any(|s| s.at == 4),
            "пока распорка не объявлена, разрыв по пробелу запрещён"
        );
        para.spacers = vec![4];
        let stops: Vec<usize> = para.opportunities().iter().map(|s| s.at).collect();
        // Разрыв встаёт НА распорку: поле коробки уходит на новую строку
        // вместе со своим текстом.
        assert_eq!(stops, vec![4]);
    }

    /// Отступ первой строки: кому он достаётся при `each-line` и `hanging`.
    #[test]
    fn indent_goes_to_the_right_lines() {
        let mut para = para("a", Wrap::default());
        let of = |p: &Paragraph, head, first| f32::from(p.indent_of(head, first, None));
        para.indent = Indent {
            px: 40.,
            ..Default::default()
        };
        assert_eq!(of(&para, true, true), 40., "первая строка блока");
        assert_eq!(of(&para, true, false), 0., "первая строка ВТОРОГО куска");
        assert_eq!(of(&para, false, true), 0., "перенесённая строка");
        para.indent.each_line = true;
        assert_eq!(of(&para, true, false), 40., "each-line: каждый кусок");
        assert_eq!(of(&para, false, true), 0., "each-line: не перенос");
        para.indent = Indent {
            px: 40.,
            hanging: true,
            ..Default::default()
        };
        assert_eq!(of(&para, true, true), 0., "hanging: кроме первой");
        assert_eq!(of(&para, false, true), 40.);
        assert_eq!(of(&para, true, false), 40.);
    }

    /// Доля берётся от ширины строки, а при замере по содержимому её нет.
    #[test]
    fn indent_share_needs_a_limit() {
        let mut para = para("a", Wrap::default());
        para.indent = Indent {
            pct: 0.1,
            ..Default::default()
        };
        assert_eq!(f32::from(para.indent_of(true, true, Some(px(300.)))), 30.);
        assert_eq!(f32::from(para.indent_of(true, true, None)), 0.);
    }
}

#[cfg(test)]
mod break_spaces_tests {
    use super::*;

    fn wrap() -> Wrap {
        Wrap {
            break_spaces: true,
            keep_spaces: true,
            ..Default::default()
        }
    }

    /// `white-space: break-spaces` даёт точку разрыва ПОСЛЕ каждого пробела.
    /// Пока их не было, строка рвалась только по правилам UAX-14, и
    /// сохранённый пробел уходил в конец строки вместо начала следующей.
    #[test]
    fn every_preserved_space_gives_a_stop() {
        let para = Paragraph::new(
            SharedString::from("X XX X".to_string()),
            vec![],
            px(25.),
            px(25.),
            Align::Left,
            wrap(),
        );
        let stops: Vec<usize> = para.opportunities().iter().map(|s| s.at).collect();
        assert!(stops.contains(&2), "после первого пробела: {stops:?}");
        assert!(stops.contains(&5), "после второго пробела: {stops:?}");
    }
}

/// Правила переноса из стиля БЕЗ вопроса, нужна ли своя раскладка.
pub fn wrap_of(c: &crate::computed::Computed) -> Wrap {
    Wrap {
        nowrap: c.nowrap == Some(true),
        break_spaces: c.break_after_spaces == Some(true),
        break_all: c.break_anywhere == Some(true) && c.break_anywhere_strict != Some(true),
        anywhere: c.break_anywhere_strict == Some(true),
        keep_all: c.keep_all == Some(true),
        break_word: c.break_word == Some(true),
        wrap_anywhere: c.wrap_anywhere == Some(true),
        rtl: c.rtl == Some(true),
        balance: c.balance_lines == Some(true),
        keep_spaces: c.keep_spaces == Some(true),
        loose: c.line_break_loose.unwrap_or(0),
        cjk_lang: c
            .lang
            .as_deref()
            .is_some_and(|l| l.starts_with("ja") || l.starts_with("zh")),
    }
}

/// Центрированная пунктуация (css-text-3 §5.2, `loose` в ja/zh): U+30FB,
/// U+FF1A, U+FF1B, U+FF65, U+203C, U+2047-2049, U+FF01, U+FF1F.
fn centered_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '\u{30FB}' | '\u{FF1A}' | '\u{FF1B}' | '\u{FF65}' | '\u{203C}' | '\u{2047}'
            ..='\u{2049}' | '\u{FF01}' | '\u{FF1F}'
    )
}

/// Постфиксы класса PO с восточноазиатской шириной A/F/W (§5.2, `loose` в
/// ja/zh): °, ‰, ℃, ％ и полноширинные знаки процента/цента.
fn wide_postfix(ch: char) -> bool {
    matches!(
        ch,
        '\u{00B0}'
            | '\u{2030}'
            | '\u{2031}'
            | '\u{2103}'
            | '\u{2109}'
            | '\u{FF05}'
            | '\u{FFE0}'
            | '\u{2032}'
            | '\u{2033}'
    )
}

/// Префиксы класса PR с шириной A/F/W (§5.2, `loose` в ja/zh): €, №, ￥, ￡,
/// ＄, ₩, §, ¶.
fn wide_prefix(ch: char) -> bool {
    matches!(
        ch,
        '\u{20AC}'
            | '\u{2116}'
            | '\u{FFE5}'
            | '\u{FFE1}'
            | '\u{FF04}'
            | '\u{FFE6}'
            | '\u{00A7}'
            | '\u{00B6}'
            | '\u{20A9}'
    )
}

/// Класс CJ по UAX-14: малая кана, знак долготы, их полуширинные формы.
/// `unicode_linebreak` разрешает CJ как NS (строгий вариант) — перед ними
/// разрыва нет; `line-break: normal`/`loose` его возвращают (css-text-3 §5.2:
/// «breaks before Japanese small kana or the Katakana-Hiragana prolonged
/// sound mark, i.e. characters from the Unicode line breaking class CJ»).
fn conditional_japanese_starter(ch: char) -> bool {
    matches!(
        ch,
        '\u{3041}' | '\u{3043}' | '\u{3045}' | '\u{3047}' | '\u{3049}' | '\u{3063}'
            | '\u{3083}' | '\u{3085}' | '\u{3087}' | '\u{308E}' | '\u{3095}' | '\u{3096}'
            | '\u{30A1}' | '\u{30A3}' | '\u{30A5}' | '\u{30A7}' | '\u{30A9}' | '\u{30C3}'
            | '\u{30E3}' | '\u{30E5}' | '\u{30E7}' | '\u{30EE}' | '\u{30F5}' | '\u{30F6}'
            | '\u{30FC}'
            | '\u{31F0}'..='\u{31FF}'
            | '\u{FF67}'..='\u{FF70}'
    )
}

/// Знаки повтора (css-text-3 §5.2, только `loose`): U+3005, U+303B, U+309D,
/// U+309E, U+30FD, U+30FE.
fn iteration_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{3005}' | '\u{303B}' | '\u{309D}' | '\u{309E}' | '\u{30FD}' | '\u{30FE}'
    )
}

/// Можно ли разорвать текст ровно на этом месте — граница ли это грозди.
///
/// Смотрятся ОБЕ стороны: знак справа не должен быть продолжением
/// (огласовка, модификатор, знак-тег), а знак слева не должен быть
/// соединителем — после нулевого соединителя гроздь продолжается следующим
/// знаком (`line-breaking-014`: радужный флаг рвался по соединителю).
fn cluster_edge(text: &str, at: usize) -> bool {
    if at >= text.len() {
        return true;
    }
    // Огласовка ПОСЛЕ пробела ни к чему не приросла: по UAX-14 (правило LB9)
    // знак-продолжение после разделителя считается обычной буквой, и рвать
    // перед ним можно.
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    if !cluster_start(&text[at..]) {
        return false;
    }
    !matches!(text[..at].chars().next_back(), Some('\u{200d}'))
}

/// То же, но с учётом `line-break: anywhere`.
///
/// `anywhere` перекрывает класс ZWJ по css-text-4, то есть рвать РЯДОМ с
/// соединителем можно. Саму гроздь он не разбирает: огласовка, знак вариации,
/// модификатор тона и знак-тег остаются при своём знаке, иначе эмодзи-цепочка
/// рассыпается по строкам (`line-breaking-014`).
fn cluster_edge_at(text: &str, at: usize, anywhere: bool) -> bool {
    if !anywhere {
        return cluster_edge(text, at);
    }
    if at >= text.len() {
        return true;
    }
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    let next = text[at..].chars().next();
    next == Some('\u{200d}') || before == Some('\u{200d}') || cluster_start(&text[at..])
}

/// Начинается ли с этого места ГРОЗДЬ знаков — то есть можно ли тут рвать.
///
/// Знаки-продолжения грозди: соединительная огласовка (класс CM по UAX-14),
/// нулевой соединитель и знаки вариации.
fn cluster_start(rest: &str) -> bool {
    let Some(ch) = rest.chars().next() else {
        return true;
    };
    // Продолжения грозди: нулевой соединитель, знаки вариации, знаки-теги
    // (флаги вроде уэльского), модификаторы тона кожи. Все они принадлежат
    // предыдущему знаку и в другую строку не уходят (`line-breaking-014`).
    if matches!(
        ch as u32,
        0x200D
            | 0xFE00..=0xFE0F
            | 0xE0100..=0xE01EF
            | 0xE0020..=0xE007F
            | 0x1F3FB..=0x1F3FF
    ) {
        return false;
    }
    !matches!(
        unicode_linebreak::break_property(ch as u32),
        unicode_linebreak::BreakClass::CombiningMark
    )
}
