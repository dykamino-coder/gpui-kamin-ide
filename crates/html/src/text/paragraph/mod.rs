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

mod atom_fit;
mod atom_placement;
mod content_baselines;
mod controlled_shape;
mod decor;
mod emphasis;
mod hyphen_shape;
mod overflow_marker;
mod ruby_justification;
mod ruby_overhang;
mod selection_geometry;
mod text_raster_origin;
mod vertical_content_baselines;
mod vertical_geometry;
mod vertical_inline;

use gpui::{AnyElement, Bounds, ElementId, Hsla, Pixels, Point, SharedString, TextRun, point, px};
mod element;
pub(crate) mod probes;
pub use crate::text::paragraph::probes::*;
pub(super) mod measure;
pub use crate::text::paragraph::measure::*;
pub(super) mod breaking;
pub use crate::text::paragraph::breaking::*;
pub(crate) mod justify;
pub(crate) use crate::text::paragraph::justify::*;
pub(super) mod runs;
use crate::text::paragraph::runs::*;
pub(super) mod geometry;
pub use crate::text::paragraph::geometry::*;
mod atoms;
mod clamp;
mod fit;
pub(super) mod paint;
use crate::text::paragraph::paint::*;

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
    /// `text-justify` opportunities besides word separators: 0 none
    /// (`inter-word`), 1 CJK ideographs (`auto`), 2 every typographic
    /// character unit (`inter-character`), see `justify_boundary`.
    justify_chars: u8,
    ruby_unit: bool,
    /// Ruby base paragraph: where to report its content width for the ruby
    /// overhang computation (`ruby_base_with_overhang`).
    ruby_base_sink: Option<std::rc::Rc<std::cell::Cell<Option<f32>>>>,
    /// `unicode-bidi: plaintext` — сторона письма выбирается для КАЖДОГО
    /// абзаца между жёсткими разрывами по его первому сильному знаку. В
    /// преформате такой абзац — это строка, поэтому и `start`/`end` у каждой
    /// строки свои (HTML ставит это правило на `dir="auto"`).
    plaintext: Option<crate::style::computed::TextAlign>,
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
    /// Знак строки обрыва `line-clamp` (`block-ellipsis`): None — U+2026,
    /// пустая строка — `no-ellipsis`.
    clamp_marker: Option<String>,
    /// (ключ клэмп-контейнера, номер абзаца): отрисовка сообщает строки
    /// вычислителю среза (`interact::publish_para_rows`).
    clamp_tag: Option<(u64, u32)>,
    /// Шаги строк абзаца ДО балансировки (`text-wrap: balance` в
    /// клэмп-контейнере, пока бюджета нет): точка среза определяется до
    /// балансировки (css-overflow-4 §line-clamp: «balancing … after
    /// the effects of continue»; `line-clamp-balance-003/006`).
    unbalanced_steps: Option<Vec<f32>>,
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
    fit: Option<crate::style::computed::TextFit>,
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
    hyphen_w: std::cell::RefCell<Vec<(usize, Pixels)>>,
    /// Куски ВНЕ потока: байтовое место в тексте → элемент. Рисуются поверх
    /// строк, места в них не занимают.
    /// Третье поле — блочный уровень: коробка встаёт в начало СЛЕДУЮЩЕЙ
    /// строки (см. `inline::Piece::Overlay`).
    overlays: Vec<(usize, AnyElement, crate::text::inline::OverlayAt)>,
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
    /// Line of a rotated vertical paragraph whose dominant baseline is central.
    rotated_central: bool,
    vertical_ccw: bool,
    selection_vertical: Option<(Bounds<Pixels>, bool)>,
    vertical_layout_origin: Point<Pixels>,
    /// Exact layout origin minus paint origin, scoped through GPUI while painting.
    glyph_nudge: Point<Pixels>,
    opaque_text_origin: bool,
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
    vertical_inline: Option<(
        crate::style::computed::orthogonal::InlineConstraint,
        Option<crate::style::computed::orthogonal::InlineKeyword>,
    )>,
    /// Предел строки для ОРТОГОНАЛЬНОГО потока: ось строки абзаца совпала с
    /// осью потока родителя, а та не ограничена. По CSS Writing Modes §7.3
    /// предел берётся от ближайшего предка-контейнера прокрутки, а при его
    /// отсутствии — от начального содержащего блока, то есть от окна.
    ortho_limit: Option<Pixels>,
    /// Какая пунктуация свисает за край (`hanging-punctuation`).
    hanging: crate::style::computed::Hanging,
    /// Отступ первой строки (`text-indent`).
    indent: Indent,
    /// Места знаков-распорок (`inline::SPACER`) — байтовые смещения по
    /// возрастанию. Точки переноса считаются по тексту без них.
    spacers: Vec<usize>,
    /// Edge spacers of inline boxes: (offset, box id, physical left, parent
    /// rtl) — see `inline::spacer_edges`.
    spacer_edges: Vec<(usize, u32, bool, bool)>,
    /// Content extents `(box id, start, end)` of those boxes.
    box_extents: Vec<(u32, usize, usize)>,
    /// Вырезы обтекания (`shape-outside`): формы слева и справа, в
    /// координатах от верха абзаца. Сужают СВОИ строки по их высоте.
    flow: std::sync::Arc<(
        Vec<crate::layout::float::shapes::FloatShape>,
        Vec<crate::layout::float::shapes::FloatShape>,
    )>,
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
    /// Shrink-to-fit data of atoms whose width depends on the containing
    /// block (CSS 2.1 §10.3.9), shared with the measure closure.
    atom_fit: std::rc::Rc<std::cell::RefCell<atom_fit::AtomFit>>,
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
    emph_spans: Vec<EmphSpan>,
    /// Куски с украшениями (`text-decoration`, css-text-decor-3 §2): линии
    /// рисует сам абзац (см. `paint_decor`), а не набор GPUI.
    decor_spans: Vec<DecorSpan>,
    /// Patterned decoration lines waiting to be merged (`decor::flush_decor`).
    decor_pending: std::cell::RefCell<Vec<decor::DecorLine>>,
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

/// Строка: что рисовать и сколько она занимает.
#[derive(Clone, Debug)]
pub(crate) struct Line {
    /// Байтовый отрезок исходного текста.
    pub(crate) range: std::ops::Range<usize>,
    /// Ширина без хвостовых пробелов — по ней идёт выключка.
    pub(crate) width: Pixels,
    /// Строка оборвана `line-clamp`: за её текстом рисуется многоточие.
    pub(crate) ellipsis: bool,
    /// Знак обрыва поставил `line-clamp` (block-ellipsis), а не
    /// `text-overflow`: у него свой знак (см. `line_mark`).
    pub(crate) clamped: bool,
    /// Усечение `text-overflow` по ВИДИМОМУ порядку (строка со смешанным
    /// направлением): ширина видимой части от начального края строки.
    /// Отрезок строки при этом целый — скрытые знаки отсекает маска.
    pub(crate) vis_cut: Option<Pixels>,
    /// Строка кончилась мягким переносом: за ней рисуется знак переноса.
    pub(crate) hyphen: bool,
    /// Отступ строки (`text-indent`). Отрицательный выводит строку за край.
    pub(crate) indent: Pixels,
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
            ruby_base_sink: take_ruby_base_sink(),
            text,
            runs,
            font_size,
            line_height,
            align,
            align_last: None,
            ruby_justify: false,
            justify_chars: 1,
            ruby_unit: false,
            plaintext: None,
            lines_reversed: false,
            letter_spacing: px(0.),
            word_spacing: px(0.),
            vertical: false,
            vertical_rl: false,
            vertical_central_baseline: true,
            rotated_central: false,
            vertical_ccw: false,
            selection_vertical: None,
            vertical_layout_origin: point(px(0.0), px(0.0)),
            glyph_nudge: point(px(0.0), px(0.0)),
            opaque_text_origin: false,
            width_nudge: px(0.0),
            indent_basis: None,
            vertical_inline: None,
            ortho_limit: None,
            hanging: crate::style::computed::Hanging::default(),
            indent: Indent::default(),
            spacers: Vec::new(),
            spacer_edges: Vec::new(),
            box_extents: Vec::new(),
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
            atom_fit: Default::default(),
            strut: (0.0, 0.0, 0.0),
            run_metrics: Vec::new(),
            edge_spans: Vec::new(),
            ruby_trim: (false, false),
            emph_spans: Vec::new(),
            decor_spans: Vec::new(),
            decor_pending: Default::default(),
            box_spans: Vec::new(),
            strut_run: None,
            strut_box: (0.0, 0.0),
            lines: Vec::new(),
            clamp: None,
            clamp_force: false,
            text_overflow: false,
            overflow_marker: None,
            clamp_marker: None,
            clamp_tag: None,
            unbalanced_steps: None,
            marker_font: None,
            marker_size: None,
            marker_color: None,
            fit: None,
            fit_spacing_scalable: true,
            fit_line_height_fixed: false,
            tab_stop: tabs::TabStops::uniform(64.0),
            hyphen: SharedString::from("\u{2010}"),
            hyphen_w: std::cell::RefCell::new(Vec::new()),
            overlays: Vec::new(),
        }
    }

    /// Пустой абзац — только чтобы на миг занять место настоящего, пока тот
    /// рисуется в повёрнутой системе координат.
    pub(crate) fn empty() -> Self {
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

    /// Куски с украшениями (см. поле `decor_spans`).
    pub fn decor_spans(mut self, spans: Vec<DecorSpan>) -> Self {
        self.decor_spans = spans;
        self
    }

    /// Куски со знаком акцента (см. поле `emph_spans`).
    pub fn emph_spans(mut self, spans: Vec<EmphSpan>) -> Self {
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
    pub fn plaintext(mut self, align: Option<crate::style::computed::TextAlign>) -> Self {
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
    pub fn hanging(mut self, hanging: Option<crate::style::computed::Hanging>) -> Self {
        self.hanging = hanging.unwrap_or_default();
        self
    }

    /// Места знаков-распорок строчных коробок.
    pub fn spacers(mut self, spacers: Vec<usize>) -> Self {
        self.spacers = spacers;
        self
    }

    /// Edge spacers with their inline boxes.
    pub fn spacer_edges(mut self, edges: Vec<(usize, u32, bool, bool)>) -> Self {
        self.spacer_edges = edges;
        self
    }

    /// Content extents of inline boxes with edge spacers.
    pub fn box_extents(mut self, extents: Vec<(u32, usize, usize)>) -> Self {
        self.box_extents = extents;
        self
    }

    /// Вырезы обтекания (`shape-outside`).
    pub fn flow_shapes(
        mut self,
        flow: std::sync::Arc<(
            Vec<crate::layout::float::shapes::FloatShape>,
            Vec<crate::layout::float::shapes::FloatShape>,
        )>,
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
    pub(crate) fn letter_spans_diverge(&self) -> bool {
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

    /// Отступ первой строки (`text-indent`).
    pub fn indent(mut self, indent: Indent) -> Self {
        self.indent = indent;
        self
    }

    /// `text-justify` mode (see `justify_chars`).
    pub fn justify_chars(mut self, mode: u8) -> Self {
        self.justify_chars = mode;
        self
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

    /// Метка абзаца в клэмп-контейнере (см. поле `clamp_tag`).
    pub fn clamp_tag(mut self, tag: Option<(u64, u32)>) -> Self {
        self.clamp_tag = tag;
        self
    }

    /// `block-ellipsis` контейнера: знак строки обрыва `line-clamp`.
    pub fn clamp_mark(mut self, mark: Option<String>) -> Self {
        self.clamp_marker = mark;
        self
    }

    /// Цвет знака обрыва — цвет блока.
    pub fn marker_color(mut self, color: Option<Hsla>) -> Self {
        self.marker_color = color;
        self
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
    pub fn overlays(
        mut self,
        overlays: Vec<(usize, AnyElement, crate::text::inline::OverlayAt)>,
    ) -> Self {
        self.overlays = overlays;
        self
    }

    /// `text-fit`: подбирать ли кегль под ширину коробки.
    pub fn text_fit(mut self, fit: Option<crate::style::computed::TextFit>) -> Self {
        self.fit = fit;
        self
    }

    /// Какие части абзаца подбор кегля вправе масштабировать.
    pub fn fit_parts(mut self, spacing_scalable: bool, line_height_fixed: bool) -> Self {
        self.fit_spacing_scalable = spacing_scalable;
        self.fit_line_height_fixed = line_height_fixed;
        self
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
