//! Описание правил промежутков и осей контейнера в точках.

/// Правила линеек одной оси (css-gaps-1), уже в точках.
#[derive(Clone, Debug)]
pub struct GapAxisRule {
    /// Ширина, цвет и видимость стиля — по промежуткам (§lists).
    pub widths: crate::style::computed::GapList<f32>,
    pub colors: crate::style::computed::GapList<crate::style::values::value::Color>,
    pub styles: crate::style::computed::GapList<bool>,
    /// §break: 0 `none`, 1 `normal`, 2 `intersection`.
    pub brk: u8,
    /// §inset: [cap-start, cap-end, junction-start, junction-end].
    pub inset: [crate::style::computed::GapInset; 4],
    /// §visibility-items: 0 `normal`, 1 `all`, 2 `around`, 3 `between`.
    pub visibility: u8,
    /// `double` style: two lines of a third of the width each, the rest a
    /// gap (as the `double` border, Blink `GetDoubleBorderStripeWidths`).
    pub double: bool,
    /// Line kind per gap: 1 `dashed`, 2 `dotted`, 3 `double`, else solid;
    /// drawn as the border side of that style (Blink `DrawBoxSide`).
    pub kinds: crate::style::computed::GapList<u8>,
}

/// Устройство контейнера для геометрии промежутков.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GapLayout {
    /// Решётка: дорожки общие для всех элементов, спаны перекрывают промежутки.
    Grid,
    /// Column boxes and spanners from the multicol fragment plan.
    Multicol,
    /// Строки гибкого контейнера или ленты: у каждой строки СВОИ промежутки
    /// между элементами. `stacked_vertically` — строки уложены сверху вниз
    /// (гибкая строка, ленты-ряды), иначе слева направо (гибкая колонка,
    /// ленты-колонки).
    Lines { stacked_vertically: bool },
}

/// Что и как рисовать в промежутках контейнера.
#[derive(Clone, Debug)]
pub struct GapRuleSpec {
    pub col: Option<GapAxisRule>,
    pub row: Option<GapAxisRule>,
    pub kind: GapLayout,
    /// Вертикальное письмо: промежутки колонок — горизонтальные полосы.
    pub vertical: bool,
    /// `rule-overlap: column-over-row`.
    pub column_over_row: bool,
    /// Зазоры в точках между x-дорожками и между y-дорожками, если известны.
    pub gap_x: Option<f32>,
    pub gap_y: Option<f32>,
    /// Размеры дорожек ШАБЛОНА в точках вдоль x и вдоль y, если весь список
    /// точечный. css-gaps-1 §gap-grid: «Row gap and column gap, in the
    /// context of a grid container, refer to the gutters between grid rows
    /// and grid columns» — промежуток задан ДОРОЖКАМИ, а не коробками
    /// элементов, и пустая дорожка остаётся дорожкой. Blink строит ту же
    /// геометрию из коллекции дорожек (`grid_layout_utils.cc`
    /// `BuildGridTrackGapData`: `LayoutGrid::ComputeExpandedPositions`), а
    /// элементы дают только занятость клетки по ИНДЕКСУ дорожки.
    pub tracks_x: Option<Vec<f32>>,
    pub tracks_y: Option<Vec<f32>>,
    /// `direction: rtl` контейнера: втяжки `*-inset-start/end` вдоль
    /// строчной оси считаются от правого края (css-gaps-1 §insets-start-end;
    /// `multicol-gap-decorations-direction-inset`, вторая половина).
    pub rtl: bool,
    /// Промежутки по x (по y) нумеруются справа налево (снизу вверх):
    /// значения списков назначаются в ЛОГИЧЕСКОМ порядке оси (css-gaps-1
    /// §assigning; эталоны `*-multi-value-direction`/`-writing-mode`).
    pub rev_x: bool,
    pub rev_y: bool,
    /// Паддинг контейнера [top, right, bottom, left] в точках: поле
    /// содержимого = паддинг-бокс художника минус он.
    pub pad: [f32; 4],
    /// Протяжённость главных промежутков строк (`GapLayout::Lines`): 0 — по
    /// элементам (многоколонник), 1 — гибкий контейнер, 2 — ленты. У гибкого
    /// главный промежуток идёт от начала поля содержимого (или первого
    /// элемента, если он левее) до конца поля содержимого (или центра
    /// последнего поперечного промежутка) — Blink `FlexGapAccumulator`
    /// (`SetContentStartOffsetsIfNeeded`, `content_main_end_ =
    /// container_main_end`); у лент — через всё поле содержимого по оси
    /// укладки («MainGaps span the final container content box (or the content
    /// when it overflows)», `grid_lanes_layout_algorithm.cc`).
    pub lines_extent: u8,
    /// Start-aligned items in stretched flex lines need the full line cross size.
    pub flex_line_stretch: bool,
    /// Ленты с явным выравниванием содержимого по оси укладки
    /// (`align-content` у колоночных лент, `justify-content` у строчных —
    /// не `normal`): главные промежутки идут лишь по выровненному
    /// содержимому, без свободного места (Blink
    /// `grid_lanes_layout_algorithm.cc`: «Explicit content alignment limits
    /// gap decoration rule bounds to the aligned content instead of including
    /// free space», `explicit_content_bounds`).
    pub lanes_content_aligned: bool,
}
