//! Прозрачная обёртка `Watch`, сообщающая узел раскладки наблюдаемой коробки. Решателю
//! пагинации нужны разложенные border-box кандидатов разрыва внутри копии. Пагинируемая
//! копия строится с рекордером `Watched` в `RenderOpts` (`FragCx::watch`); `render::element`
//! оборачивает элемент, чей id рекордер наблюдает, а обёртка записывает внутренний узел
//! раскладки и отдаёт его как свой — taffy видит то же дерево. Узлы читает `seen`, `reset`
//! забывает их перед замером. Рекордер блочной стопки (`for_blocks`) наблюдает коробки
//! `flow::blocks::BlockBox` главной копии ребёнка стопки и после её раскладки отдаёт точки
//! разрыва по разложенной геометрии (`breaks`): border box коробок, дорожки рядов сеток
//! (`DetailedLayoutInfo::Grid` разложенной сетки, читается у её элемента) и таблицы стопки
//! кадрами (`table`: коробка таблицы, сетка и дорожки рядов).

use crate::layout::fragment::flow::blocks::{BlockBox, BlockBreaks, MeasuredBreaks};
use crate::layout::fragment::geometry::Node;
use crate::layout::fragment::types::StackAxis;
use gpui::{LayoutId, Pixels, Point, Size, Window};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

mod axis;
mod construct;
mod hook;
mod monolithic;
pub(crate) use hook::wrap;
use hook::{Slot, Watch};
mod flex;
mod flex_flows;
mod flex_geometry;
mod geometry;
mod grid;
mod owner;
mod table;
use table::TableNodes;
pub(crate) use table::{TablePart, table_part};

/// Break candidates of one paginated child and the layout nodes its last
/// measuring layout gave them. Owned by the child's `Paginate`, shared with
/// the wrappers of its rebuilt copies.
#[derive(Default)]
pub(crate) struct Watched {
    pub(crate) flow: Rc<crate::layout::fragment::flow::FlowMeasure>,
    ids: Rc<HashSet<u64>>,
    seen: Rc<RefCell<HashMap<u64, Node>>>,
    /// Block stack of the watched copy whose boundaries are read after layout.
    blocks: Option<BlockBox>,
    /// Keys of the tables of `blocks` (`layout::table::grid_rows::table_key`).
    tables: Rc<HashSet<u64>>,
    /// Layout nodes of the watched tables' parts by the last layout.
    table_nodes: Rc<RefCell<HashMap<u64, TableNodes>>>,
    copies: RefCell<HashMap<usize, Rc<Self>>>,
    independent: bool,
    axis: Option<StackAxis>,
    flex_growth: Rc<RefCell<HashMap<u64, Vec<f32>>>>,
    flex_offsets: Rc<RefCell<HashMap<u64, f32>>>,
    flex_consumed: Rc<RefCell<HashMap<u64, (f32, bool)>>>,
    item_flows: Rc<HashMap<u64, Rc<crate::layout::fragment::flow::FlowMeasure>>>,
    /// Линии элементов колонки flex, закреплённые до фрагментации (`flex`).
    flex_lines: Rc<RefCell<HashMap<u64, usize>>>,
    /// Номер копии-фрагмента, которую строит этот рекордер (`owner`).
    part: usize,
    /// Первый фрагмент каждого flex-элемента по плану стопки (`owner`).
    paint_owners: Rc<RefCell<HashMap<u64, usize>>>,
    /// Маска предка окна фрагмента на время краски копии (`owner`).
    paint_outer: Rc<RefCell<Option<gpui::ContentMask<Pixels>>>>,
    /// Flex-контейнеры `blocks`: раскладываются в контексте фрагментации.
    flex_ids: std::cell::OnceCell<HashSet<u64>>,
    monolithic_ids: std::cell::OnceCell<HashSet<u64>>,
}

impl Watched {
    pub(super) fn axis(&self) -> StackAxis {
        self.axis.unwrap_or(StackAxis::Horizontal)
    }

    /// Whether the recorder watches a block stack (`for_blocks`).
    pub(crate) fn has_blocks(&self) -> bool {
        self.blocks.is_some()
    }

    /// Break opportunities of the copy laid out at `origin` with `size`, read
    /// before the layout engine is released: measured lines
    /// (`FlowMeasure`) and the block stack's class A boundaries, forced
    /// breaks and unbreakable ranges from the laid-out border boxes
    /// (`BlockBox::breaks`). `end` bounds the flow (the box or its parallel
    /// flow). Границы блоков переводятся в логическую блочную ось.
    pub(crate) fn breaks(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        end: f32,
        axis: StackAxis,
        window: &mut Window,
    ) -> MeasuredBreaks {
        let lines = self.flow.at(origin, size, axis, window);
        let blocks = self.blocks.as_ref().map_or_else(BlockBreaks::default, |b| {
            b.breaks(end, &self.laid_axis(origin, size, axis, window))
        });
        blocks.finish(lines, end)
    }

    /// Block stack watched by the recorder (`for_blocks`).
    pub(crate) fn blocks(&self) -> Option<&BlockBox> {
        self.blocks.as_ref()
    }

    /// Recorder for the inside of a box whose line boxes are not lines of
    /// the watched flow (`flow::flow_lines`): the same watched boxes, but its
    /// lines and float hosts go to a flow nobody reads.
    pub(crate) fn nested(&self) -> Rc<Self> {
        Rc::new(Self {
            flow: Rc::default(),
            ids: self.ids.clone(),
            seen: self.seen.clone(),
            blocks: None,
            // Table parts are laid out inside the table box, under this
            // recorder: their nodes belong to the watched flow's tables.
            tables: self.tables.clone(),
            table_nodes: self.table_nodes.clone(),
            flex_growth: self.flex_growth.clone(),
            flex_offsets: self.flex_offsets.clone(),
            flex_consumed: self.flex_consumed.clone(),
            item_flows: self.item_flows.clone(),
            copies: RefCell::default(),
            independent: self.independent,
            part: self.part,
            flex_lines: self.flex_lines.clone(),
            paint_owners: self.paint_owners.clone(),
            paint_outer: self.paint_outer.clone(),
            // Вложенные flex-контейнеры — тоже из стопки родителя.
            flex_ids: self.flex_ids_of().clone().into(),
            axis: self.axis,
            monolithic_ids: self.monolithic_ids_of().clone().into(),
        })
    }

    /// Whether this recorder takes line boxes (`nested` does not).
    pub(crate) fn takes_lines(&self) -> bool {
        self.flow.lines
    }

    /// Forget recorded nodes before a measuring layout.
    pub(crate) fn reset(&self) {
        self.seen.borrow_mut().clear();
        self.table_nodes.borrow_mut().clear();
        self.flow.reset();
        for flow in self.item_flows.values() {
            flow.reset();
        }
    }

    pub(crate) fn space(&self, window: &Window) -> Option<gpui::LayoutSpace> {
        let root = self.blocks.as_ref()?;
        Some(self.seen.borrow().get(&root.id)?.read(window)?.space)
    }

    pub(crate) fn normal(&self, id: u64, window: &Window) -> Option<gpui::Bounds<Pixels>> {
        self.seen.borrow().get(&id)?.bounds(window)
    }

    /// Layout node recorded for `id` by the last layout.
    pub(crate) fn seen(&self, id: u64, window: &Window) -> Option<LayoutId> {
        self.seen.borrow().get(&id)?.id_in(window)
    }
}

impl std::fmt::Debug for Watched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watched")
            .field("ids", &self.ids.len())
            .finish()
    }
}
