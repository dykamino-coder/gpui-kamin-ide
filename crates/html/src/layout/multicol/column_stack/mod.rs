//! Элемент стопки колонок `ColumnStack`.
// owner: A

use crate::layout::fragment::types::{
    Frag, Intrinsic, Kid, RepeatGeom, Rows, StackAxis, StackChild,
};
use gpui::{Bounds, IntoElement, Pixels};

pub mod avoid;
mod axis;
pub mod balance;
pub mod element;
pub mod fill;

pub struct ColumnStack {
    pub(crate) children: Vec<StackChild>,
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
    /// `column-fill: auto` + заданная высота: заполнение без баланса.
    pub(crate) fixed_height: Option<f32>,
    /// `fixed_height` пришёл из `max-height` при авто-высоте: колонки
    /// заполняются до него, а коробка — по самой высокой занятой колонке
    /// (Blink `ConstrainColumnBlockSize`; `columnfill-auto-max-height-003`).
    /// Ставится `with_fill_shrink`.
    pub(crate) fill_shrink: bool,
    /// Линейка между колонками: ширина и цвет.
    pub(crate) rule: Option<(f32, gpui::Hsla)>,
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
    pub(crate) gap_items: Option<crate::paint::gap_rules::GapItems>,
    pub(crate) plan: std::cell::RefCell<Vec<Frag>>,
    pub(crate) col_w: std::cell::Cell<f32>,
    /// Линии колонок после укладки: `(y, высота)` каждой — линейкам и
    /// смещениям. Колонка `col` стоит в линии `col / count`. Без спаннеров
    /// линия = ряд; спаннер режет ряд на линии (Blink `LayoutLine`).
    pub(crate) lines_plan: std::cell::RefCell<Vec<(f32, f32)>>,
    /// Спаннеры после укладки: `(ребёнок, y)`.
    pub(crate) spans_plan: std::cell::RefCell<Vec<(usize, f32)>>,
}

impl ColumnStack {
    pub fn new(
        children: Vec<StackChild>,
        count: usize,
        gap: f32,
        fixed_height: Option<f32>,
        rule: Option<(f32, gpui::Hsla)>,
        rows: Option<Rows>,
        gap_items: Option<crate::paint::gap_rules::GapItems>,
        intrinsic: Option<Intrinsic>,
    ) -> Self {
        // Без рядов копий ровно столько, сколько колонок (как прежде); с
        // рядами — сколько построил `render.rs`: ребёнок может занять
        // больше колонок, чем `column-count`.
        let copies = match rows {
            Some(_) => children
                .iter()
                .map(|c| c.frags.len() + 1)
                .max()
                .unwrap_or(1),
            // Переполняющие колонки (css-multicol-1 §8.2) при `column-fill: auto`
            // с заданной высотой: копий столько, сколько построил `render.rs`
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
            children,
            count: count.max(1),
            gap,
            fixed_height,
            fill_shrink: false,
            rule,
            rule_to: None,
            rows,
            axis: StackAxis::Horizontal,
            row_phase: 0.0,
            copies,
            gap_items,
            intrinsic,
            plan: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Коробка по самой высокой занятой колонке (см. `fill_shrink`).
    pub(crate) fn with_fill_shrink(mut self, on: bool) -> Self {
        self.fill_shrink = on;
        self
    }

    /// Блочный размер СОДЕРЖИМОГО многоколоночника рядами `rows` (та же
    /// укладка, что у `request_layout`): мера вложенного многоколоночника для
    /// внешней стопки (`render::nested_rows_shape`).
    pub(crate) fn measure_rows(
        kids: &[Kid],
        count: usize,
        gap: f32,
        fixed: Option<f32>,
        rows: Rows,
    ) -> f32 {
        let mut probe =
            ColumnStack::new(Vec::new(), count, gap, fixed, None, Some(rows), None, None);
        probe.row_phase = 0.0;
        // Копий — как у `render.rs` для стопки с рядами: сколько колонок
        // ребёнок может занять, с запасом на поля и срезы.
        let per = rows.h.unwrap_or(f32::MAX).max(1.0);
        let span = kids
            .iter()
            .map(|k| (k.h / per).ceil() as usize)
            .max()
            .unwrap_or(0);
        probe.copies = (span + 2).max(count).min(48);
        // Блочный размер — низ ПОСЛЕДНЕЙ линии, а не полный ряд: последний
        // фрагмент сбалансирован и короче ряда (Blink `LayoutRow`:
        // `intrinsic_block_size_` растёт на высоту строки колонок;
        // `multicol-breaking-006`: ряд 100 + хвост 80 + рамка 20).
        let (h, lines, _, _) = probe.balance(kids);
        lines
            .iter()
            .map(|l| l.0 + l.1)
            .fold(0.0f32, f32::max)
            .min(h)
            .max(0.0)
    }

    /// Смещение начала рядов (`row_phase`).
    /// Блочный размер содержимого коробки заданной высоты: линейки
    /// единственной линии колонок тянутся до него (Blink `PaintColumnRules`:
    /// «Paint column rules as tall as the entire multicol container, but only
    /// when at the last row»).
    pub fn with_rule_stretch(mut self, h: Option<f32>) -> Self {
        self.rule_to = h;
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
        self.children
            .iter()
            .map(|c| Kid {
                h: c.h,
                mt: c.mt,
                mb: c.mb,
                monolith: c.monolith,
                cuts: c.cuts.clone(),
                force_before: c.force_before,
                force_after: c.force_after,
                avoid_before: c.avoid_before,
                avoid_after: c.avoid_after,
                forced: c.forced.clone(),
                solid: c.solid.clone(),
                span: c.span,
                over: c.over,
                clone_dec: c.clone_dec,
                overflow_top: c.overflow_top,
                repeat: c.repeat.as_ref().map_or(RepeatGeom::default(), |r| r.geom),
                par: c.par,
            })
            .collect()
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

impl IntoElement for ColumnStack {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
