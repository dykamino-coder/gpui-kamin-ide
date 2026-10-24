//! Состояние продолжения по контексту форматирования (`FcState`): где внутри блока, строк,
//! flex, grid, таблицы и многоколоночника возобновить раскладку в следующем фрагментаинере.
//! Только индексы и величины; дети, прервавшиеся вместе с коробкой, — `TokenId` в хранилище.

use super::TokenId;
use crate::layout::block::struts::Strut;

/// Прерванный ребёнок: номер среди детей коробки (в порядке, которым их
/// обходит её алгоритм) и его собственный токен.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChildToken {
    pub(crate) idx: u32,
    pub(crate) token: TokenId,
}

/// Сторона флоата.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FloatSide {
    InlineStart,
    InlineEnd,
}

/// Флоат, начатый раньше и ещё занимающий место в новом фрагментаинере
/// (css-break-3 §3 parallel flows): строки и блоки обтекают его отсюда.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FloatExclusion {
    pub(crate) side: FloatSide,
    /// Строчный размер полосы с полем.
    pub(crate) inline_size: f32,
    /// Конец полосы от верха нового фрагментаинера.
    pub(crate) block_end: f32,
}

/// Блочный поток (css-break-3 §5.5; Blink `BlockBreakToken`
/// `ChildBreakTokens`, Gecko `nsBlockFrame` overflow lines).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BlockState {
    /// Размер normal content, относительно которого накоплен consumed у leaf.
    /// При смене геометрии страницы сохраняется доля пройденного (css-break-3 §5.1).
    pub(crate) content_block_size: Option<f32>,
    /// Доли пройденных начального/концевого краёв slice; не входят в consumed.
    pub(crate) sliced_edges: Option<(f32, f32)>,
    /// Прерванный normal-flow ребёнок при `child=Some`, иначе первый ещё не начатый.
    /// Более ранние независимые продолжения находятся только в `parallel`.
    pub(crate) child_idx: u32,
    /// Поля, схлопнутые до разрыва (CSS 2.1 §8.3.1): усекаются на
    /// непринудительном разрыве и сохраняются на принудительном (css-break-3 §5.2).
    pub(crate) margin_strut: Strut,
    pub(crate) float_exclusions: Vec<FloatExclusion>,
    /// Токен ребёнка `child_idx`, прерванного внутри себя.
    pub(crate) child: Option<TokenId>,
    /// Параллельные потоки: флоаты и переполнение коробок с заданной высотой
    /// продолжаются сами по себе (Blink `IsInParallelFlow`), по возрастанию `idx`.
    pub(crate) parallel: Vec<ChildToken>,
}

/// Строки (Blink `InlineBreakToken`, Gecko `nsLineBox` + `PushLines`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InlineState {
    /// Номер первой строки продолжения среди строк абзаца.
    pub(crate) line_idx: usize,
    /// Байтовое смещение начала строки в тексте абзаца (Blink `StartTextOffset`).
    pub(crate) text_pos: usize,
    /// Первая форматированная строка уже пройдена: `::first-line` и
    /// `text-indent` больше не применяются (Blink `IsPastFirstFormattedLine`).
    pub(crate) past_first_line: bool,
    /// Начало после обязательного текстового переноса (`text-indent: each-line`).
    pub(crate) after_hard_break: bool,
    /// Блок внутри inline; атомы, ruby и first-letter принадлежат целой строке.
    pub(crate) child: Option<TokenId>,
}

/// Flex (css-flexbox-1 §12; Blink `FlexBreakTokenData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FlexState {
    /// Первая flex-строка (у колонки — элемент), не уложенная целиком.
    pub(crate) line: u32,
    /// Элементы этой строки, прерванные внутри себя: строка — параллельные потоки.
    pub(crate) items: Vec<ChildToken>,
    /// Накопленный рост строк, когда элемент сдвинут за разрыв и строка
    /// растягивается до него (Blink `FlexLine::item_offset_adjustment`).
    pub(crate) offset_adjust: f32,
}

/// Grid (css-grid-2 §11; Blink `GridBreakTokenData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GridState {
    /// Первая дорожка-ряд продолжения.
    pub(crate) row_track: u32,
    pub(crate) items: Vec<ChildToken>,
    /// Сдвиг рядов от растянутых разрывом элементов (Blink `row_offset_adjustments`).
    pub(crate) offset_adjust: f32,
}

/// Где таблица относительно своей коробки рядов (Blink `TableBreakTokenData`
/// `has_entered_table_box` / `is_past_table_box`): подписи — вне её.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TablePhase {
    #[default]
    BeforeBox,
    InBox,
    PastBox,
}

/// Таблица (css-tables-3 §6.2 fragmentation; Blink `TableBreakTokenData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableState {
    pub(crate) phase: TablePhase,
    pub(crate) section: u32,
    /// Ряд внутри секции.
    pub(crate) row: u32,
    /// Повторить шапку/подвал в следующем фрагментаинере (Blink:
    /// повтор, пока полоса ≤ ¼ фрагментаинера).
    pub(crate) repeat_header: bool,
    pub(crate) repeat_footer: bool,
    /// Ячейки ряда, прерванные внутри себя.
    pub(crate) cells: Vec<ChildToken>,
    /// Пройденная часть коробки рядов (Blink `consumed_table_box_block_size`).
    pub(crate) consumed_box: f32,
}

/// Многоколоночник внутри фрагментаинера (css-multicol-1 §8; Blink — токен
/// контейнера с флагом `IsCausedByColumnSpanner`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MulticolState {
    /// Ряд колонок, с которого продолжить (вложенный ряд — `Kind::NestedRow`).
    pub(crate) row: u32,
    /// Спаннер, перед которым или на котором прервались.
    pub(crate) spanner: Option<u32>,
    /// Токен потока колонок.
    pub(crate) flow: Option<TokenId>,
}

/// Состояние контекста форматирования коробки на разрыве.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FcState {
    Block(BlockState),
    Inline(InlineState),
    Flex(FlexState),
    Grid(GridState),
    Table(TableState),
    Multicol(MulticolState),
}

impl FcState {
    /// Токены детей, на которые ссылается это состояние, в порядке обхода.
    pub(crate) fn child_tokens(&self) -> impl Iterator<Item = TokenId> + '_ {
        let (one, kids): (_, &[ChildToken]) = match self {
            FcState::Block(b) => (b.child, &b.parallel),
            FcState::Inline(i) => (i.child, &[]),
            FcState::Flex(f) => (None, &f.items),
            FcState::Grid(g) => (None, &g.items),
            FcState::Table(t) => (None, &t.cells),
            FcState::Multicol(m) => (m.flow, &[]),
        };
        one.into_iter().chain(kids.iter().map(|c| c.token))
    }
}
