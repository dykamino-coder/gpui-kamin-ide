//! Пространство фрагментаинера (`Space`): вид контекста фрагментации, логические размеры,
//! начало фрагментаинера, повтор и личность страницы. Чистые данные без `gpui::Window`:
//! их получают и экранные стопки, и печатный драйвер.

use crate::layout::fragment::types::StackAxis;
use crate::style::computed::writing_mode::WritingMode;

/// Привлекательность разрыва (css-break-3 §4.4 «classes of break points»,
/// Blink `break_appeal.h`): порядок вариантов — порядок предпочтения, худший
/// первым. Gecko выражает то же через `nsReflowStatus` и повтор reflow с
/// запретом разрыва; отдельной шкалы там нет.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BreakAppeal {
    /// Не точка класса A/B/C: соблюдена лишь целость монолита.
    #[default]
    LastResort,
    /// Нарушен `break-*: avoid`.
    ViolatingAvoid,
    /// Нарушены `orphans`/`widows`.
    ViolatingOrphansWidows,
    /// Законная точка без нарушений.
    Perfect,
}

/// Вид контекста фрагментации (css-break-3 §2 «fragmentation context»;
/// Blink `FragmentationType`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    /// Колонки многоколоночника.
    Columns,
    /// Page areas (печать или стопка страниц на экране).
    Pages,
    /// Ряд колонок вложенного многоколоночника внутри внешнего фрагментаинера.
    NestedRow(NestedRow),
}

/// Ряд вложенного многоколоночника (css-break-4 §2.1: «when a multi-column
/// container breaks across pages, it generates a new row of columns on the
/// next page»; Blink `column_layout_algorithm.cc` `ConstrainColumnBlockSize`).
/// Сегодня то же несёт `FragCx::outer_row` как `(высота, фаза)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NestedRow {
    /// Блочный размер внешнего фрагментаинера — высота полного ряда.
    pub(crate) outer_block: f32,
    /// Где во внешнем фрагментаинере начат вложенный контейнер (Blink
    /// `FragmentainerOffset`): первый ряд получает остаток `outer_block − offset`.
    pub(crate) offset: f32,
    /// Внешний контекст — страницы (печать), а не колонки.
    pub(crate) outer_paged: bool,
}

impl NestedRow {
    /// Из пары `FragCx::outer_row` `(высота ряда, фаза)`.
    pub(crate) fn from_outer_row((outer_block, offset): (f32, f32), outer_paged: bool) -> Self {
        NestedRow {
            outer_block,
            offset: offset.max(0.0),
            outer_paged,
        }
    }

    /// Блочный размер ряда `row`: первый — остаток внешнего фрагментаинера.
    pub(crate) fn row_block(self, row: usize) -> f32 {
        if row == 0 {
            (self.outer_block - self.offset).max(0.0)
        } else {
            self.outer_block
        }
    }
}

/// Повторяемое содержимое (Blink `ShouldRepeat`/`IsInsideRepeatableContent`,
/// `constraint_space.h`): шапка/подвал таблицы (css-tables-3 §6.2 repeat),
/// `position: fixed` при печати (css-position-3 §3.4 «once on every page»).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RepeatState {
    #[default]
    None,
    /// Коробка этого пространства повторяется в каждом фрагментаинере; `last` —
    /// известно, что повтор последний.
    Repeat { last: bool },
    /// Потомок повторяемой коробки: разрывов внутри нет.
    Inside,
}

/// Левая или правая страница (css-page-3 §3.3 «Page Progression»).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageSide {
    Left,
    Right,
}

impl PageSide {
    /// Сторона страницы `index` (с нуля, пустые страницы `break-before: left|right`
    /// входят в счёт): при прогрессии слева направо первая — правая. Прогрессия
    /// — строчное направление у горизонтального письма и блочное у вертикального.
    pub(crate) fn of(index: u32, principal: WritingMode) -> Self {
        let rtl = if principal.vertical() {
            principal.block_rl()
        } else {
            principal.rtl()
        };
        if index.is_multiple_of(2) != rtl {
            PageSide::Right
        } else {
            PageSide::Left
        }
    }
}

/// Личность страницы для `@page` (css-page-3 §3 selectors, §4 `page`): номер,
/// именная страница (номер в таблице имён драйвера страниц), сторона, пустая ли.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageIdentity {
    pub(crate) index: u32,
    pub(crate) name: Option<u32>,
    pub(crate) side: PageSide,
    /// Вставлена ради `break-before: left|right` (`:blank`).
    pub(crate) blank: bool,
}

impl PageIdentity {
    /// `:first` — первая напечатанная страница.
    pub(crate) fn is_first(self) -> bool {
        self.index == 0
    }
}

/// Фрагментаинер, в который продолжается коробка (Blink `ConstraintSpace`
/// fragmentation fields; Gecko — доступный блочный размер `ReflowInput`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Space {
    /// Режим письма контекста: оси логические, физика — при сборке.
    pub(crate) wm: WritingMode,
    /// Строчный размер фрагментаинера; `None` — его ещё задаёт вызывающий.
    pub(crate) inline_size: Option<f32>,
    /// Блочный размер (Blink `FragmentainerBlockSize`); `None` — неизвестен
    /// (начальный проход балансировки колонок).
    pub(crate) block_size: Option<f32>,
    pub(crate) kind: Kind,
    /// Сколько первых фрагментаинеров начинаются не с верха (Blink
    /// `IsAtFragmentainerStart` ложно): монолит оттуда переносится, а не
    /// переполняет (css-break-3 §4.1 «may be pushed»).
    pub(crate) not_at_start: usize,
    /// Нижняя граница привлекательности разрыва (Blink `MinBreakAppeal`):
    /// вложенный ряд при уже начатом внешнем фрагментаинере.
    pub(crate) min_appeal: BreakAppeal,
    pub(crate) repeat: RepeatState,
    /// Только у страниц.
    pub(crate) page: Option<PageIdentity>,
}

impl Space {
    fn of_kind(kind: Kind) -> Self {
        Space {
            wm: WritingMode::default(),
            inline_size: None,
            block_size: None,
            kind,
            not_at_start: 0,
            min_appeal: BreakAppeal::LastResort,
            repeat: RepeatState::None,
            page: None,
        }
    }

    pub(crate) fn columns() -> Self {
        Self::of_kind(Kind::Columns)
    }

    pub(crate) fn pages() -> Self {
        Self::of_kind(Kind::Pages)
    }

    pub(crate) fn nested_row(row: NestedRow) -> Self {
        Space {
            block_size: Some(row.row_block(0)),
            not_at_start: usize::from(row.offset > 0.0),
            ..Self::of_kind(Kind::NestedRow(row))
        }
    }

    pub(crate) fn with_not_at_start(self, not_at_start: usize) -> Self {
        Space {
            not_at_start,
            ..self
        }
    }

    /// Ось стопки фрагментаинеров по режиму письма.
    pub(crate) fn axis(self) -> StackAxis {
        StackAxis::of(self.wm)
    }

    /// Печать: монолит, переполнивший страницу, занимает место и на следующих
    /// (Blink `IsPaginated` и `MonolithicOverflow`, crbug 1402540). Вложенный
    /// ряд — колонки, даже на странице (Blink: тип ближайшего контекста).
    pub(crate) fn is_paginated(self) -> bool {
        matches!(self.kind, Kind::Pages)
    }

    /// Начальный проход балансировки (Blink `IsInitialColumnBalancingPass`).
    pub(crate) fn is_initial_balancing_pass(self) -> bool {
        matches!(self.kind, Kind::Columns) && self.block_size.is_none()
    }

    /// Фрагментаинер `index` начинается с верха.
    pub(crate) fn at_start(self, index: usize) -> bool {
        index >= self.not_at_start
    }

    /// Можно ли рвать внутри коробки этого пространства: повторяемое
    /// содержимое монолитно (Blink `IsMonolithic` для `ShouldRepeat`).
    pub(crate) fn may_break_inside(self) -> bool {
        self.repeat == RepeatState::None
    }

    /// Пространство вложенного многоколоночника, начатого на `offset` этого
    /// фрагментаинера; `None`, пока блочный размер неизвестен.
    pub(crate) fn nest_at(self, offset: f32) -> Option<Space> {
        let outer_block = self.block_size?;
        let outer_paged = match self.kind {
            Kind::Pages => true,
            Kind::Columns => false,
            Kind::NestedRow(row) => row.outer_paged,
        };
        let row = NestedRow::from_outer_row((outer_block, offset), outer_paged);
        Some(Space {
            wm: self.wm,
            inline_size: self.inline_size,
            ..Space::nested_row(row)
        })
    }

    /// Ряд колонок при уже начатом внешнем фрагментаинере не рвёт хуже, чем
    /// худший разрыв предыдущих колонок ряда (`floor` — их минимум): остальное
    /// уходит в следующий внешний фрагментаинер (Blink `column_layout_algorithm.cc`
    /// `min_break_appeal` при `may_have_more_space_in_next_outer_fragmentainer`).
    pub(crate) fn with_min_appeal(self, floor: BreakAppeal) -> Self {
        Space {
            min_appeal: floor,
            ..self
        }
    }
}
