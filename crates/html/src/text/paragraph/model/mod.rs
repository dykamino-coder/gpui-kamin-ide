//! Model for paragraph; split out to keep the owning module within 250 lines.

use crate::text::paragraph::{atom_fit, decor, tabs};
mod configuration;
mod construct;
mod overflow_config;

use super::{Align, Indent, Line, Wrap};
pub use crate::text::paragraph::probes::*;
use gpui::{AnyElement, Bounds, ElementId, Hsla, Pixels, Point, SharedString, TextRun};

/// Абзац со своей раскладкой строк.
pub struct Paragraph {
    pub(super) text: SharedString,
    pub(super) runs: Vec<TextRun>,
    pub(super) font_size: Pixels,
    pub(super) line_height: Pixels,
    pub(super) align: Align,
    /// Выключка последней строки (`text-align-last`), если задана.
    pub(super) align_last: Option<Align>,
    pub(super) ruby_justify: bool,
    /// `text-justify` opportunities besides word separators: 0 none
    /// (`inter-word`), 1 CJK ideographs (`auto`), 2 every typographic
    /// character unit (`inter-character`), see `justify_boundary`.
    pub(super) justify_chars: u8,
    pub(super) ruby_unit: bool,
    /// Ruby base paragraph: where to report its content width for the ruby
    /// overhang computation (`ruby_base_with_overhang`).
    pub(super) ruby_base_sink: Option<std::rc::Rc<std::cell::Cell<Option<f32>>>>,
    /// `unicode-bidi: plaintext` — сторона письма выбирается для КАЖДОГО
    /// абзаца между жёсткими разрывами по его первому сильному знаку. В
    /// преформате такой абзац — это строка, поэтому и `start`/`end` у каждой
    /// строки свои (HTML ставит это правило на `dir="auto"`).
    pub(super) plaintext: Option<crate::style::computed::TextAlign>,
    /// Строки в ОБРАТНОМ порядке (снизу вверх): у `vertical-lr` колонки идут
    /// слева направо, а поворот по часовой кладёт ПЕРВУЮ строку правой —
    /// подача снизу вверх возвращает ей левую колонку.
    pub(super) lines_reversed: bool,
    /// `line-clamp`: сколько строк показывать, остальные обрываются.
    pub(super) clamp: Option<usize>,
    /// Знак обрыва положен, даже если абзац влез в предел целиком: точка
    /// среза стоит СРАЗУ ЗА ним, между блоками (css-overflow-4 §5.3 —
    /// знак привязан к точке среза, а не к тому, что абзац не поместился).
    /// Только авто-режим; счётный путь сюда не заходит.
    pub(super) clamp_force: bool,
    /// `text-overflow: ellipsis` контейнера с обрезкой.
    pub(super) text_overflow: bool,
    /// Маркер обрезки вместо многоточия (`text-overflow: <string>`).
    pub(super) overflow_marker: Option<String>,
    /// Знак строки обрыва `line-clamp` (`block-ellipsis`): None — U+2026,
    /// пустая строка — `no-ellipsis`.
    pub(super) clamp_marker: Option<String>,
    /// (ключ клэмп-контейнера, номер абзаца): отрисовка сообщает строки
    /// вычислителю среза (`interact::publish_para_rows`).
    pub(super) clamp_tag: Option<(u64, u32)>,
    /// Шаги строк абзаца ДО балансировки (`text-wrap: balance` в
    /// клэмп-контейнере, пока бюджета нет): точка среза определяется до
    /// балансировки (css-overflow-4 §line-clamp: «balancing … after
    /// the effects of continue»; `line-clamp-balance-003/006`).
    pub(super) unbalanced_steps: Option<Vec<f32>>,
    /// Шрифт маркера: стиль БЛОКА-контейнера, не прогона у среза
    /// (css-overflow-4 §5) — иначе «123» набиралось Ahem-квадратами
    /// шрифта обрезанного куска.
    pub(super) marker_font: Option<gpui::Font>,
    /// Кегль маркера: тоже БЛОЧНЫЙ. Базовый кегль абзаца — это `biggest`,
    /// кегль САМОГО КРУПНОГО куска; снятие собственного кегля прогона
    /// (`run.font_size = None`) отдаёт маркеру именно его, а не кегль
    /// блока, и строка-замена внутри `<span style="font-size:30px">`
    /// мерилась втрое шире нужного (`text-overflow-string-003…026`).
    pub(super) marker_size: Option<Pixels>,
    /// Цвет знака обрыва — цвет БЛОКА (css-overflow-4 §5.3: знак — анонимный
    /// строчный ребёнок блока, а не куска у среза; `block-ellipsis-005`).
    pub(super) marker_color: Option<Hsla>,
    /// `text-fit`: подбор кегля под ширину коробки.
    pub(super) fit: Option<crate::style::computed::TextFit>,
    /// Масштабируемые части подбора кегля (css-text-5 §text-fit): интервалы
    /// в ДОЛЯХ кегля масштабируются вместе с ним, в точках и `em` — нет
    /// (`em` считается от вычисленного кегля, а его подбор не трогает).
    pub(super) fit_spacing_scalable: bool,
    /// Заданная `line-height` (длина) при подборе не меняется; `normal` и
    /// число — считаются от использованного кегля и растут с ним.
    pub(super) fit_line_height_fixed: bool,
    /// Шаг позиций табуляции (`tab-size` в точках).
    pub(super) tab_stop: tabs::TabStops,
    /// Чем показывать перенос слова (`hyphenate-character`).
    pub(super) hyphen: SharedString,
    /// Ширина этого знака — считается при раскладке, где есть окно.
    pub(super) hyphen_w: std::cell::RefCell<Vec<(usize, Pixels)>>,
    /// Куски ВНЕ потока: байтовое место в тексте → элемент. Рисуются поверх
    /// строк, места в них не занимают.
    /// Третье поле — блочный уровень: коробка встаёт в начало СЛЕДУЮЩЕЙ
    /// строки (см. `inline::Piece::Overlay`).
    pub(super) overlays: Vec<(usize, AnyElement, crate::text::inline::OverlayAt)>,
    /// Трекинг (`letter-spacing`): добавка к каждому знаку.
    pub(super) letter_spacing: Pixels,
    /// `word-spacing` — добавка к КАЖДОМУ пробелу. Шейпер о ней не знает,
    /// поэтому она добавляется к положению знака: сколько пробелов позади,
    /// столько добавок.
    pub(super) word_spacing: Pixels,
    /// Вертикальное письмо: строка идёт СВЕРХУ ВНИЗ, а строки набегают по
    /// горизонтали. Абзац при этом остаётся обычным элементом раскладки —
    /// ограничение приходит от родителя по нужной оси, а не выдумывается.
    pub(super) vertical: bool,
    /// `vertical-rl` — строки набегают справа налево.
    pub(super) vertical_rl: bool,
    /// Dominant baseline in the rotated frame; sideways uses the real alphabetic baseline.
    pub(super) vertical_central_baseline: bool,
    /// Line of a rotated vertical paragraph whose dominant baseline is central.
    pub(super) rotated_central: bool,
    pub(super) vertical_ccw: bool,
    pub(super) selection_vertical: Option<(Bounds<Pixels>, bool)>,
    pub(super) vertical_layout_origin: Point<Pixels>,
    /// Exact layout origin minus paint origin, scoped through GPUI while painting.
    pub(super) glyph_nudge: Point<Pixels>,
    pub(super) opaque_text_origin: bool,
    /// Exact (unsnapped) inline size minus the snapped one: alignment
    /// (`text-align: right/center`, rtl start) is measured from the exact
    /// edges, so a right-aligned glyph ends on the box's exact right edge.
    pub(super) width_nudge: Pixels,
    /// Exact (unsnapped) inline size of the box at paint time: the basis of
    /// a percentage `text-indent` (css-text-3 §8.1: percentage of the
    /// containing block's inline size). The paint-time line limit is the
    /// snapped size plus one device pixel of slack, so a 50% indent landed
    /// half a device pixel off (`text-indent-103`).
    pub(super) indent_basis: Option<Pixels>,
    pub(super) vertical_inline: Option<(
        crate::style::computed::orthogonal::InlineConstraint,
        Option<crate::style::computed::orthogonal::InlineKeyword>,
    )>,
    /// Предел строки для ОРТОГОНАЛЬНОГО потока: ось строки абзаца совпала с
    /// осью потока родителя, а та не ограничена. По CSS Writing Modes §7.3
    /// предел берётся от ближайшего предка-контейнера прокрутки, а при его
    /// отсутствии — от начального содержащего блока, то есть от окна.
    pub(super) ortho_limit: Option<Pixels>,
    /// Какая пунктуация свисает за край (`hanging-punctuation`).
    pub(super) hanging: crate::style::computed::Hanging,
    /// Отступ первой строки (`text-indent`).
    pub(super) indent: Indent,
    /// Места знаков-распорок (`inline::SPACER`) — байтовые смещения по
    /// возрастанию. Точки переноса считаются по тексту без них.
    pub(super) spacers: Vec<usize>,
    /// Edge spacers of inline boxes: (offset, box id, physical left, parent
    /// rtl) — see `inline::spacer_edges`.
    pub(super) spacer_edges: Vec<(usize, u32, bool, bool)>,
    /// Content extents `(box id, start, end)` of those boxes.
    pub(super) box_extents: Vec<(u32, usize, usize)>,
    /// Вырезы обтекания (`shape-outside`): формы слева и справа, в
    /// координатах от верха абзаца. Сужают СВОИ строки по их высоте.
    pub(super) flow: std::sync::Arc<(
        Vec<crate::layout::float::shapes::FloatShape>,
        Vec<crate::layout::float::shapes::FloatShape>,
    )>,
    /// Опознание абзаца для памяти выделения. Без него абзац не выделяется:
    /// состояние между кадрами хранит раскладка по этому ключу.
    pub(super) id: Option<ElementId>,
    /// Цвет подложки выделенного куска.
    pub(super) highlight: Hsla,
    pub(super) wrap: Wrap,
    /// Правила переноса ПО КУСКАМ: `word-break` или `overflow-wrap`, заданные
    /// на вложенном `<span>`, действуют только на его байты. Пусто — значит
    /// весь абзац живёт по одному правилу.
    pub(super) spans: Vec<(std::ops::Range<usize>, Wrap)>,
    /// Межсловный интервал ПО КУСКАМ: `word-spacing` на вложенном `<span>`
    /// действует только на пробелы внутри него. Пусто — значит на весь абзац
    /// один интервал.
    pub(super) word_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Трекинг ПО КУСКАМ: `letter-spacing` на вложенном `<span>` действует
    /// только на его знаки. Набор принимает трекинг скаляром, поэтому разница
    /// с общим значением добавляется к положению знака, а слово набирается
    /// своим трекингом.
    pub(super) letter_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Сдвиг куска по вертикали (`vertical-align: super`/`sub`): смещение
    /// базовой линии в точках, вниз положительное.
    pub(super) shift_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Своя `line-height` куска (§10.8): строка растёт до полулидинга самого
    /// высокого куска. Отдельно от `line_height` — та принадлежит блоку.
    pub(super) lh_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Относительный сдвиг кусков (CSS 2.1 §9.4.3): двигает только
    /// отрисовку. Отдельно от `shift_spans` — тот растит строчную коробку,
    /// а этот на поток не влияет вовсе.
    pub(super) rel_spans: Vec<(std::ops::Range<usize>, (f32, f32))>,
    /// Атомарные строчные коробки В СТРОКЕ (CSS 2.1 §9.2.2, §10.8): место в
    /// тексте держит распорка (U+FEFF), её продвижение — ширина атома, а сам
    /// атом раскладывается отдельно и ставится на базовую линию своей строки.
    pub(super) atoms: Vec<AtomSlot>,
    /// Замеры атомов (ширина, высота, базовая линия) — копируются в щуп
    /// замера, сами элементы туда не уходят.
    pub(super) atom_boxes: Vec<AtomBox>,
    /// Shrink-to-fit data of atoms whose width depends on the containing
    /// block (CSS 2.1 §10.3.9), shared with the measure closure.
    pub(super) atom_fit: std::rc::Rc<std::cell::RefCell<atom_fit::AtomFit>>,
    /// Метрики струта абзаца (§10.8.1): подъём, спуск и x-высота первого
    /// прогона — от них считается, насколько атом вылезает за строку.
    pub(super) strut: (f32, f32, f32),
    /// Подъём и спуск ОСНОВНОГО шрифта каждого прогона: по ним строка с
    /// кусками разного кегля ставит их на ОДНУ базовую линию (§10.8) — так же,
    /// как сплошной набор строки (`ShapedLine::paint` берёт наибольшие подъём
    /// и спуск строки).
    pub(super) run_metrics: Vec<(f32, f32)>,
    /// Куски с `vertical-align: top`/`bottom` (§10.8.1): отрезок байт, край
    /// (`true` — верх) и высота их строчной коробки (`line-height` куска).
    /// Равняются по краю ГОТОВОЙ строки и растят её, только если выше неё.
    pub(super) edge_spans: Vec<(std::ops::Range<usize>, bool, f32)>,
    /// `text-box-trim` блока (начало, конец): аннотация руби и знак акцента
    /// первой строки сверху и последней снизу её не растят — срез идёт по краю текста
    /// корневой строчной коробки, и выход аннотации срезался бы всё равно
    /// (css-inline-3 §text-box-trim, `text-box-trim-ruby-start-001`).
    pub(super) ruby_trim: (bool, bool),
    /// Куски со знаком акцента (`text-emphasis`): (отрезок, снизу?, высота
    /// знака). Знак растит строку так же, как аннотация руби
    /// (css-text-decor-3 §5.3 «If emphasis marks … do not fit, the UA must
    /// increase the line height»): эталоны семьи `text-emphasis-line-height-*`
    /// строятся именно из руби.
    pub(super) emph_spans: Vec<EmphSpan>,
    /// Куски с украшениями (`text-decoration`, css-text-decor-3 §2): линии
    /// рисует сам абзац (см. `paint_decor`), а не набор GPUI.
    pub(super) decor_spans: Vec<DecorSpan>,
    /// Patterned decoration lines waiting to be merged (`decor::flush_decor`).
    pub(super) decor_pending: std::cell::RefCell<Vec<decor::DecorLine>>,
    /// Строчные коробки кусков ПОСТРОЧНО (CSS 2.1 §10.8.1): отрезок байт →
    /// `line-height` куска в точках. Непусто — у абзаца куски разного кегля,
    /// гарнитуры или высоты строки, и `line_height` абзаца — это СТРУТ блока,
    /// а каждая строка собирается из коробок своих кусков: верх — наибольший
    /// подъём с полулидингом, низ — наибольший спуск. Прежде на весь абзац
    /// шла одна высота по самому крупному куску.
    pub(super) box_spans: Vec<(std::ops::Range<usize>, f32)>,
    /// Шрифт и кегль струта (шрифт самого блока) — для `box_spans`.
    pub(super) strut_run: Option<(gpui::Font, Pixels)>,
    /// Подъём и спуск струта в точках (меряются в `request_layout`).
    pub(super) strut_box: (f32, f32),
    /// Границы строк в байтах — считаются на замере, переиспользуются на
    /// отрисовке.
    pub(super) lines: Vec<Line>,
}
