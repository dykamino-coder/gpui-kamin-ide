//! Стопка колонок `ColumnStack` — GPUI-элемент многоколоночного контейнера: геометрия детей
//! (`StackChild`), колонки, зазор, ось, ряды, линейка и буферы плана после укладки; сама
//! укладка — в подмодулях `balance`, `fill`, `avoid`, `paginate`, `element`.

use crate::layout::fragment::plan::Plan;
use crate::layout::fragment::types::{Intrinsic, Kid, Rows, StackAxis, StackChild};
use gpui::{Bounds, Pixels};

pub mod avoid;
mod axis;
pub mod balance;
pub mod element;
mod gap_geometry;
pub(crate) mod oof;
mod paginate;

pub struct ColumnStack {
    pub(crate) paint: std::rc::Rc<crate::layout::cx::deferred::Deferred>,
    pub(crate) children: Vec<StackChild>,
    pub(crate) deferred: Option<std::rc::Rc<super::deferred::Recipe>>,
    pub(crate) count: usize,
    pub(crate) gap: f32,
    /// Ось прогрессии колонок (`StackAxis`); при вертикальном письме —
    /// вертикальная. Ставится `with_axis`.
    pub(crate) axis: StackAxis,
    /// Смещение начала рядов: вложенный многоколоночник начат на `row_phase`
    /// ниже верха внешней колонки, и его ПЕРВЫЙ ряд — остаток `h − row_phase`
    /// (Blink `column_layout_algorithm.cc:818-828` `available_outer_space =
    /// FragmentainerSpaceLeftForChildren() − line_offset`). Ставится
    /// `with_row_phase`; ноль — ряды от верха.
    pub(crate) row_phase: f32,
    pub(crate) row_extent: super::measure::RowExtent,
    /// `column-fill: auto` + заданная высота: заполнение без баланса.
    pub(crate) fixed_height: Option<f32>,
    /// Высота выбранной пробы: regrow и paginate раскладывают ребёнка
    /// относительно этой сетки; `None` запускает поиск баланса.
    pub(crate) forced_h: Option<f32>,
    pub(crate) balance_failure: Option<crate::layout::fragment::token::Error>,
    /// `fixed_height` пришёл из `max-height` при авто-высоте: колонки
    /// заполняются до него, а коробка — по самой высокой занятой колонке
    /// (Blink `ConstrainColumnBlockSize`; `columnfill-auto-max-height-003`).
    /// Ставится `with_fill_shrink`.
    pub(crate) fill_shrink: bool,
    pub(crate) rule_to: Option<f32>,
    /// Ряды колонок; `None` — одна линия, как в css-multicol-1.
    pub(crate) rows: Option<Rows>,
    /// Сколько копий у ребёнка (= сколько колонок он может занять).
    pub(crate) copies: usize,
    /// Внутренние размеры контейнера, если его ширину решает СОДЕРЖИМОЕ.
    /// `None` — ширину даёт родитель, и мерить детей незачем: лишний проход
    /// раскладки стоит дороже, чем всё остальное в этом элементе.
    pub(crate) intrinsic: Option<Intrinsic>,
    /// Буфер границ колонок и спаннеров для `GapRulePainter` (css-gaps-1
    /// §gap-multicol): колонки линии — элементы строки, спаннер — элемент во
    /// всю ширину. Заполняется в `prepaint`, художник забирает в `paint`.
    pub(crate) gap_items: Option<crate::layout::multicol::gap_rules::GapItems>,
    pub(crate) plan: std::cell::RefCell<Plan>,
    pub(super) oof: oof::State,
    /// Reuse placement storage while mapping viewport static-position probes.
    pub(crate) static_places: Option<crate::layout::fragment::static_coordinates::Places>,
    pub(crate) col_w: std::cell::Cell<f32>,
    /// Линии колонок после укладки: `(y, высота)` каждой — линейкам и
    /// смещениям. Колонка `col` стоит в линии `col / count`. Без спаннеров
    /// линия = ряд; спаннер режет ряд на линии (Blink `LayoutLine`).
    pub(crate) lines_plan: std::cell::RefCell<Vec<(f32, f32)>>,
    /// Спаннеры после укладки: `(ребёнок, y)`.
    pub(crate) spans_plan: std::cell::RefCell<Vec<(usize, f32)>>,
}

impl ColumnStack {
    // Construction keeps the independent CSS column constraints explicit.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        children: Vec<StackChild>,
        count: usize,
        gap: f32,
        fixed_height: Option<f32>,
        rows: Option<Rows>,
        gap_items: Option<crate::layout::multicol::gap_rules::GapItems>,
        intrinsic: Option<Intrinsic>,
        paint: std::rc::Rc<crate::layout::cx::deferred::Deferred>,
    ) -> Self {
        // Без рядов копий ровно столько, сколько колонок (как прежде); с
        // рядами — сколько построил `render::generic_box::multicol`: ребёнок может занять
        // больше колонок, чем `column-count`.
        let copies = match rows {
            Some(_) => children
                .iter()
                .map(|c| c.frags.len() + 1)
                .max()
                .unwrap_or(1),
            // Переполняющие колонки (css-multicol-1 §8.2) при `column-fill: auto`
            // с заданной высотой: копий столько, сколько построил `render::generic_box::multicol`
            // (лишние он строит только ребёнку с абсолютным потомком). Без
            // такого ребёнка у всех детей ровно `count` копий, и значение
            // тождественно прежнему. Балансировку это не трогает: без
            // `fixed_height` ветка ниже, как прежде.
            None if fixed_height.is_some() => children
                .iter()
                .map(|c| c.frags.len() + 1)
                .max()
                .unwrap_or(1)
                .max(count.max(1)),
            None => count.max(1),
        };
        ColumnStack {
            paint,
            children,
            deferred: None,
            count: count.max(1),
            gap,
            fixed_height,
            forced_h: None,
            balance_failure: None,
            fill_shrink: false,
            rule_to: None,
            rows,
            axis: StackAxis::Horizontal,
            row_phase: 0.0,
            row_extent: Default::default(),
            copies,
            gap_items,
            intrinsic,
            plan: std::cell::RefCell::new(Plan::default()),
            oof: oof::State::default(),
            static_places: Default::default(),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn with_deferred(
        mut self,
        recipe: Option<std::rc::Rc<super::deferred::Recipe>>,
    ) -> Self {
        self.deferred = recipe;
        self
    }

    /// Коробка по самой высокой занятой колонке (см. `fill_shrink`).
    pub(crate) fn with_fill_shrink(mut self, on: bool) -> Self {
        self.fill_shrink = on;
        self
    }

    /// Линейки единственной линии колонок до заданного блочного размера.
    pub fn with_rule_stretch(mut self, h: Option<f32>) -> Self {
        self.rule_to = h;
        self
    }

    pub(crate) fn with_row_extent(mut self, extent: super::measure::RowExtent) -> Self {
        self.row_extent = extent;
        self
    }

    pub fn with_row_phase(mut self, phase: f32) -> Self {
        self.row_phase = phase.max(0.0);
        self
    }

    /// Ось прогрессии колонок (см. `StackAxis`).
    pub fn with_axis(mut self, axis: StackAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Геометрия детей для укладки (то же, что строят `request_layout` и
    /// `prepaint` горизонтальной стопки).
    pub(crate) fn kid_geoms(&self) -> Vec<Kid> {
        self.children.iter().map(StackChild::kid).collect()
    }

    /// Строчный и блочный размеры коробки стопки по её оси.
    pub(crate) fn axis_sizes(&self, bounds: Bounds<Pixels>) -> (f32, f32) {
        if self.axis.is_vertical() {
            (f32::from(bounds.size.height), f32::from(bounds.size.width))
        } else {
            (f32::from(bounds.size.width), f32::from(bounds.size.height))
        }
    }

    /// Где стоит колонка `col`: номер в линии и смещение линии. Без рядов и
    /// при `nowrap` колонки идут вбок сплошь (переполняющие — за край).
    pub(crate) fn place(&self, col: usize) -> (usize, f32) {
        match self.rows {
            Some(r) if r.wrap => {
                let lines = self.lines_plan.borrow();
                (
                    col % self.count,
                    lines.get(col / self.count).map_or(0.0, |l| l.0),
                )
            }
            _ => (col, 0.0),
        }
    }
}
