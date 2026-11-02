//! CSS-операнды группы context; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetContext {
    /// Собственные значения CSS до объединения обязательных видов обособления.
    pub contain_value: super::Containment,
    /// Внутренняя копия прокручиваемой коробки (`render` снимает с неё
    /// `overflow`, чтобы обёртка `ScrollArea` резала сама): по спеке она
    /// остаётся scroll container — автоминимум по `aspect-ratio` к ней не
    /// применяется (css-sizing-4 §5.2; `block-aspect-ratio-011/012`).
    pub scroller: bool,
    /// `visibility: collapse` — не «невидимый», а ВЫБРОШЕННЫЙ из строки
    /// гибкого контейнера: перенос и размеры считаются без него.
    pub collapsed: Option<bool>,
    /// Ортогональный элемент СЕТКИ с невытягивающим выравниванием
    /// (`place-items: start` и родня): по строчной оси он размером в
    /// содержимое, а не в область сетки (css-grid-1 §6.6, css-align-3 §6.1).
    /// Ставится сборщиком детей сетки, глубже не наследуется.
    pub hug_inline: bool,
    /// Родитель — ВЕРТИКАЛЬНАЯ сетка с невытягивающим `justify-*` (или блок
    /// по цепочке под ней, `layout::writing_mode::vertical_hug`): повёрнутый
    /// абзац заявляет высотой длину своей строки. Отдельно от `hug_inline`:
    /// ортогональным элементам горизонтальной сетки (и лункам) такая заявка
    /// противопоказана (`grid-lanes/.../column-explicit-placement-002`
    /// при общем флаге ломается).
    pub hug_claim: bool,
    /// Ортогональный (вертикальный) блок в потоке горизонтального блочного
    /// контейнера: baseline-набор родителя его не берёт (css-writing-modes-4
    /// §4.3, css-align-3 §9.1 «baseline-export»; Blink пропускает
    /// ортогональных детей). Ставится сборщиком детей контейнера.
    pub baseline_orthogonal: bool,
    /// `table-layout: fixed` — колонки равной ширины, без замера содержимого.
    pub table_fixed: Option<bool>,
    /// `animation` — ссылка на набор кадров.
    pub animation: Option<AnimSpec>,
    /// Easing also applies when declared on a keyframe.
    pub animation_easing: Option<crate::style::animation::easing::Easing>,
    /// `transition` — длительность перехода в секундах.
    pub transition: Option<f32>,
    /// Unemitted adjoining margin at the float's source position (CSS 2.1 §9.5.1).
    pub float_margin_offset: Option<f32>,
    /// The box is a float whose `float` a float row/host cleared for layout:
    /// it never gives an enclosing inline-block its baseline (CSS 2.1 §10.8.1).
    pub floated_box: bool,
    /// Explicit `clear: inherit`; ordinary `clear` is non-inherited.
    pub clear_inherit: bool,
    /// Ограничение ОРТОГОНАЛЬНОГО потока: определённый размер ближайшего
    /// предка-контейнера прокрутки по оси потока (CSS Writing Modes §7.3).
    /// Наследуется вниз, потому что искать его надо ВВЕРХ по дереву, а на
    /// момент раскладки ребёнка предков уже не видно.
    pub ortho_limit: Option<f32>,
    /// Содержимое ближайшего предка — контейнера запросов размера — по
    /// физическим осям (ширина, высота): база единиц `cqw`/`cqh`/`cqi`/…
    /// (css-conditional-5 §container-lengths). Ось без такого контейнера
    /// (или с неизвестным размером) — `None`, и единица берёт малое окно.
    /// Наследуется вниз: контейнер ищется ВВЕРХ по дереву.
    pub cq_box: (Option<f32>, Option<f32>),
    /// Used content box этого контейнера: явный layout input следующего прохода стиля.
    pub cq_size: (Option<f32>, Option<f32>),
    /// Nearest ancestor scrollport, including an indefinite nearest scroller.
    pub orthogonal_scrollport: Option<[orthogonal::AxisSizes; 2]>,
    /// Used inline measurement contract of an ordinary orthogonal block.
    pub orthogonal_inline: Option<orthogonal::InlineConstraint>,
    /// Ячейка таблицы, ПАРАЛЛЕЛЬНОЙ своему письму: доступное инлайн-место у
    /// неё ОПРЕДЕЛЕНО — это мера её КОЛОНКИ (css-tables-3
    /// §computing-column-measures), — и запасной предел §7.3
    /// (`ortho_limit`) применять нельзя: тот стоит на месте НЕОПРЕДЕЛЁННОГО
    /// инлайн-места (css-writing-modes-4 §7.3.1, «an additional constraint is
    /// used as a fallback in place of the available inline space»). Blink
    /// делит эти случаи ровно так же: `space_utils.h:36
    /// SetOrthogonalFallbackInlineSizeIfNeeded` выходит НЕ СДЕЛАВ НИЧЕГО при
    /// `IsParallelWritingMode(таблица, ячейка)`, а
    /// `table_layout_utils.cc:1363 SetupTableCellConstraintSpaceBuilder`
    /// кладёт ячейке `SetAvailableSize({cell_inline_size, …})`, где
    /// `cell_inline_size` собран из `column_locations` — то есть из дорожки.
    /// Флаг ставит `table()` своим ячейкам; ГЛУБЖЕ НЕ НАСЛЕДУЕТСЯ (как
    /// `hug_inline`): `style::cascade::inherit` начинает с `own.clone()`, а у
    /// вложенного элемента поле пусто.
    pub ortho_col: bool,
    /// Physical paragraph of a `text-orientation: upright` vertical stack
    /// (its clone clears `vertical`). Not inherited, like `ortho_col`.
    pub upright_stack: bool,
    /// Повёрнутый абзац `vertical-lr`: строки-колонки идут слева направо —
    /// подача строк снизу вверх (см. `Paragraph::reversed_lines`).
    pub lines_reversed: Option<bool>,
    /// Edge spacer of an inline box (`inline::SPACER`): (box id, physical
    /// left edge, the box's own direction is rtl). Bidi reordering moves the edges to
    /// the box's visually outermost fragments (CSS 2.1 §8.6).
    pub spacer_edge: Option<(u32, bool, bool)>,
    /// Абзац вертикального письма набирается САМ, по оси строки решённой
    /// раскладкой (`lines::Paragraph::vertical`): `Some(rl)`. Ставится только
    /// на копию стиля внутри `render::paragraph`.
    pub para_vertical: Option<bool>,
    /// Логические стороны и размеры до перевода в физические.
    pub logical: Option<Box<Logical>>,
    /// Вычисленное `user-select` (`None` — начальное `auto`); не наследуется.
    pub user_select: Option<crate::style::computed::UserSelect>,
    /// Используемое `user-select` — текст не выделяется (`Some(true)`);
    /// `auto` решается по коробке-родителю (`inherit::box_stage`).
    pub no_select: Option<bool>,
    /// `frame-sizing: content-height` (css-sizing-4
    /// §frame-sizing): высота `<iframe>` — по содержимому вложенного
    /// документа, если он сам согласился (`<meta name=responsive-embedded-sizing>`).
    pub frame_sizing_height: bool,
    /// `overflow-clip-margin`: на сколько обрезка отступает НАРУЖУ от коробки.
    /// Письмо `<body>` НЕ стало письмом области просмотра: обособление есть
    /// либо на `<html>`, либо на самом `<body>`, и распространение свойств
    /// тела наружу выключено (css-contain-2 §containment-types: «when any
    /// containments are active on either the HTML html or body elements,
    /// propagation of properties from the body element to the initial
    /// containing block, the viewport, or the canvas background, is
    /// disabled»). Вычисленное письмо тела при этом остаётся при нём
    /// (css-writing-modes §3.1 — распространяется только used корневой
    /// коробки), поэтому отличить главный поток по одному лишь письму тела
    /// нельзя, и признак приходится нести пометкой. Ставится сборкой
    /// документа, не каскадом.
    pub wm_contained: bool,
    /// Коробка КОРНЯ документа: её содержащий блок — начальный, и высота его
    /// определена всегда (§10.5). Ставится вместе с пометкой канваса, чтобы
    /// снимаемая обёртка уносила признак с собой.
    pub root_box: bool,
    /// ОПРЕДЕЛЕНА ли высота содержащего блока: от неё зависит, считается ли
    /// доля высоты вообще (§10.5 — иначе значение равно `auto`).
    pub cb_height_def_hint: bool,
    /// Режим quirks: высота, от которой ДЕТИ этой коробки решают долю
    /// высоты при неопределённом содержащем блоке (Quirks Mode §3.5 «The
    /// percentage height calculation quirk» — ближайший предок с не-`auto`
    /// высотой). `None` — квирка нет или опоры не нашлось.
    pub quirk_pct_base_hint: Option<f32>,
    /// Floats of this ordinary block reach nothing after it (`dom::float_tail`):
    /// its float host may use in-flow auto height (CSS 2.1 §10.6.3).
    pub float_tail: bool,
    /// Довод `fit-content(<length-percentage>)` у `width`, `min-width`,
    /// `max-width` (по порядку); само значение остаётся `Len::FitContent`.
    pub fit_arg: [Option<Len>; 3],
    /// Intrinsic min/max constraints rewritten as a preferred keyword retain their sizing wrapper.
    pub intrinsic_wrapper_required: bool,
    /// `contain: paint` — содержимое обрезается по коробке.
    pub contain_paint: Option<bool>,
    /// `contain: size` — коробка меряется ПУСТОЙ (css-contain-1 §3):
    /// размер задают явные свойства и `contain-intrinsic-size`.
    pub contain_size: Option<bool>,
    /// `contain: inline-size` — обособлена только СТРОЧНАЯ ось
    /// (css-contain-2 §inline-size): содержимое не влияет на неё, но
    /// блочную ось по-прежнему задаёт.
    pub contain_inline_size: Option<bool>,
    /// `contain: layout|content` — независимый контекст форматирования.
    pub contain_layout: Option<bool>,
    /// `display: flow-root` — свой контекст форматирования (коробка Block).
    pub flow_root: Option<bool>,
    /// `display: inline` дословно (не inline-block): §9.7/§10.2 дорешиваются
    /// после каскада — см. `dom::finish_inline_display`.
    pub inline_display: Option<bool>,
    /// `display: run-in` — вбегание решает `dom::fold_run_ins`.
    pub run_in: Option<bool>,
    /// CSS ancestor clipping inside a continuous fragmentation copy.
    pub fragment_clip_ancestor: bool,
    /// `display: table-caption` — метка для таблицы.
    pub is_caption: Option<bool>,
    /// Род группы рядов: 0 — шапка, 1 — тело, 2 — подвал. `Display` у всех
    /// трёх ОДИН (`TableRowGroup`), раскладка у них одинаковая, — а §17.5.3
    /// требует переставить шапку вверх, подвал вниз. Различить их по
    /// `Display` нечем, поэтому род хранится отдельно.
    pub row_group_kind: Option<u8>,
    /// Колоночная роль: 0 — `display: table-column`, 1 —
    /// `table-column-group`. `Display` при этом ОСТАЁТСЯ `None`: коробки
    /// колонка не порождает (§17.2.1), и весь поток обходит её ровно как
    /// раньше. Метка — единственное, что сохраняет узел в дереве: из него
    /// берутся ширина дорожки (§17.5.2.1), слой краски (§17.5.1) и рамка
    /// для разбора сросшихся кромок (§17.6.2.1).
    pub col_role: Option<u8>,
    /// `contain: style` — счётчики и кавычки не выходят из поддерева.
    pub contain_style: Option<bool>,
    /// `contain-intrinsic-size`: подменная своя величина (css-sizing-5 §5).
    pub contain_intrinsic: (Option<f32>, Option<f32>),
    /// `content-visibility: hidden` — детей не собирать вовсе.
    pub skip_content: Option<bool>,
    /// Блок, вынесенный расщеплением строчного хозяина (block-in-inline,
    /// `render::blocks`): в дереве отрисовки он брат хозяина, а по DOM — его
    /// ребёнок. Объёмный контекст и перспектива деда на него не действуют:
    /// плоский строчный хозяин — лист контекста, поддерево сплющивается в его
    /// плоскость (css-transforms-2 §3d-rendering-context; §perspective — только
    /// прямые дети). Ставится при выносе, читает `render::transformed`.
    pub hoisted_block: bool,
    /// `appearance` (css-ui-4 §7.2); `None` is the initial `auto`.
    pub appearance: Option<crate::style::computed::Appearance>,
    /// Generated inline controls have no bytes in first-line source offsets.
    pub inline_control: bool,
    /// A piece of a float-band host (`layout::float::band_flow_host`): the host resolves its
    /// percentages against the band layout and places it; not inherited.
    pub band_kid: bool,
    /// The style of the initial containing block (`RenderOpts::root_style`),
    /// whose box is the embedder's; not inherited.
    pub icb: bool,
}
