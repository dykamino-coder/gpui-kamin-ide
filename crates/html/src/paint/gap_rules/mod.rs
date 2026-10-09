//! Покраска правил промежутков.
// owner: A

use gpui::{AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Window, px};

pub mod geometry;
pub mod painter;

mod gap_segments;
mod gap_fragment_tail;

/// Прямоугольники элементов сетки/гибкого контейнера: их собирают пробы
/// детей, а по ним слой-художник считает середины промежутков
/// (css-gaps-1 §geometry: линейка идёт по ЦЕНТРАЛЬНОЙ ЛИНИИ промежутка).
pub type GapItems = std::rc::Rc<std::cell::RefCell<Vec<Bounds<Pixels>>>>;

thread_local! {
    static GAP_ITEMS: std::cell::RefCell<std::collections::HashMap<u64, GapItems>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Стек контейнеров с линейками промежутков при ПОСТРОЕНИИ дерева.
    /// Ровно как `CLAMP_STACK`: проба ставится только НЕПОСРЕДСТВЕННЫМ
    /// детям, поэтому сторож кладёт ключ на время сборки детей.
    static GAP_STACK: std::cell::RefCell<Vec<u64>> = std::cell::RefCell::new(Vec::new());
}

pub fn gap_items_for(key: u64) -> GapItems {
    GAP_ITEMS.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

pub fn forget_gap_buffers() {
    GAP_ITEMS.with(|m| m.borrow_mut().clear());
    GAP_STACK.with(|st| st.borrow_mut().clear());
}

/// Сторож стека линеек на время сборки ДЕТЕЙ контейнера.
pub struct GapGuard(pub(crate) bool);

impl GapGuard {
    pub fn enter(key: u64) -> Self {
        GAP_STACK.with(|st| st.borrow_mut().push(key));
        GapGuard(true)
    }
}

impl Drop for GapGuard {
    fn drop(&mut self) {
        if self.0 {
            GAP_STACK.with(|st| {
                st.borrow_mut().pop();
            });
        }
    }
}

/// Ключ контейнера линеек, в котором строится текущий элемент.
pub fn gap_context() -> Option<u64> {
    GAP_STACK.with(|st| st.borrow().last().copied())
}

/// Проба элемента сетки: как `edge_probe`, но пишет только границы.
///
/// Абсолютная проба `top: 0; left: 0; size: 100%` ложится на ПАДДИНГ-бокс
/// элемента (содержащий блок абсолютного потомка — padding box, CSS 2.1
/// §10.1), а геометрия промежутков строится по РАМОЧНЫМ коробкам элементов
/// (css-gaps-1 §gap-grid/§gap-flex: промежуток — между краями элементов,
/// Blink `GapGeometry` берёт border-box фрагментов). `border` — толщины
/// рамки элемента [top, right, bottom, left]: на них проба расширяется
/// (`grid-gap-decorations-008`: элементы с `border: 1px`, линейки вставали
/// на 1px внутрь от первой и последней линии сетки).
pub fn gap_item_probe(items: GapItems, border: [f32; 4]) -> AnyElement {
    GapItemProbe { items, border }.into_any_element()
}

/// Элемент пробы: границы берутся БЕЗ округления к точке устройства.
/// Линейка ставится по середине промежутка между элементами, а эталон
/// кладёт её абсолютной коробкой от точного начала (`top: 64.17px`) —
/// от округлённых краёв элементов середина уезжает на долю точки, и при
/// масштабе 1.25 край линейки округлялся на строку ниже
/// (`flex-gap-decorations-048`: строка y=86 лишняя).
struct GapItemProbe {
    pub(crate) items: GapItems,
    pub(crate) border: [f32; 4],
}

impl Element for GapItemProbe {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let origin = window.layout_origin_unrounded(*state);
        let size = window.layout_size_unrounded(*state);
        let [t, r, b, l] = self.border;
        self.items.borrow_mut().push(Bounds {
            origin: gpui::point(origin.x - px(l), origin.y - px(t)),
            size: gpui::size(size.width + px(l + r), size.height + px(t + b)),
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for GapItemProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

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
}

/// Устройство контейнера для геометрии промежутков.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GapLayout {
    /// Решётка: дорожки общие для всех элементов, спаны перекрывают промежутки.
    Grid,
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
    /// Ленты с явным выравниванием содержимого по оси укладки
    /// (`align-content` у колоночных лент, `justify-content` у строчных —
    /// не `normal`): главные промежутки идут лишь по выровненному
    /// содержимому, без свободного места (Blink
    /// `grid_lanes_layout_algorithm.cc`: «Explicit content alignment limits
    /// gap decoration rule bounds to the aligned content instead of including
    /// free space», `explicit_content_bounds`).
    pub lanes_content_aligned: bool,
}
