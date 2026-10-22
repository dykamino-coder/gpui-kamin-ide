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

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::fragment::flow::blocks::{BlockBox, BlockBreaks, Laid, MeasuredBreaks};
use crate::layout::fragment::types::StackAxis;
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Point, Size, Window};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

mod table;
use table::TableNodes;
pub(crate) use table::{TablePart, table_part};

/// Break candidates of one paginated child and the layout nodes its last
/// measuring layout gave them. Owned by the child's `Paginate`, shared with
/// the wrappers of its rebuilt copies.
#[derive(Default)]
pub(crate) struct Watched {
    pub(crate) flow: Rc<crate::layout::fragment::flow::FlowMeasurements>,
    ids: Rc<HashSet<u64>>,
    seen: Rc<RefCell<HashMap<u64, LayoutId>>>,
    /// Block stack of the watched copy whose boundaries are read after layout.
    blocks: Option<BlockBox>,
    /// Keys of the tables of `blocks` (`layout::table::grid_rows::table_key`).
    tables: Rc<HashSet<u64>>,
    /// Layout nodes of the watched tables' parts by the last layout.
    table_nodes: Rc<RefCell<HashMap<u64, TableNodes>>>,
}

impl Watched {
    pub(crate) fn new(ids: &[u64]) -> Rc<Self> {
        Rc::new(Watched {
            ids: Rc::new(ids.iter().copied().collect()),
            seen: Rc::default(),
            flow: Rc::default(),
            ..Self::default()
        })
    }

    /// Recorder of a stack child's main copy: measured line edges plus the
    /// border boxes of its block stack `blocks`.
    pub(crate) fn for_blocks(blocks: BlockBox) -> Rc<Self> {
        Rc::new(Watched {
            // With the growth struts before them (`STRUT_ID`).
            ids: Rc::new(
                blocks
                    .ids()
                    .into_iter()
                    .flat_map(|id| [id, id ^ crate::layout::fragment::flow::blocks::STRUT_ID])
                    .collect(),
            ),
            seen: Rc::default(),
            tables: Rc::new(blocks.table_keys().into_iter().collect()),
            flow: Rc::new(crate::layout::fragment::flow::FlowMeasurements::for_lines()),
            blocks: Some(blocks),
            ..Self::default()
        })
    }

    /// Whether the recorder watches a block stack (`for_blocks`).
    pub(crate) fn has_blocks(&self) -> bool {
        self.blocks.is_some()
    }

    /// Break opportunities of the copy laid out at `origin` with `size`, read
    /// before the layout engine is released: measured lines
    /// (`FlowMeasurements`) and the block stack's class A boundaries, forced
    /// breaks and unbreakable ranges from the laid-out border boxes
    /// (`BlockBox::breaks`). `end` bounds the flow (the box or its parallel
    /// flow). Block boundaries are read on the horizontal axis only.
    pub(crate) fn breaks(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        end: f32,
        axis: StackAxis,
        window: &mut Window,
    ) -> MeasuredBreaks {
        let lines = self.flow.at(origin, size, axis, window);
        let blocks = match (&self.blocks, axis) {
            (Some(b), StackAxis::Horizontal) => b.breaks(end, &self.laid(origin, window)),
            _ => BlockBreaks::default(),
        };
        blocks.finish(lines, end)
    }

    /// Laid-out geometry of the copy relative to `origin`, read before the
    /// layout engine is released: border boxes `(top, bottom)` of the watched
    /// boxes and frames of the watched tables.
    pub(crate) fn laid(&self, origin: Point<Pixels>, window: &mut Window) -> Laid {
        let boxes = self
            .seen
            .borrow()
            .iter()
            .map(|(&id, &lid)| {
                let top = f32::from(window.layout_origin_unrounded(lid).y - origin.y);
                let h = f32::from(window.layout_size_unrounded(lid).height);
                (id, (top, top + h))
            })
            .collect();
        Laid {
            boxes,
            tables: self.table_frames(origin, window),
            grids: self.rows(origin, window),
        }
    }

    /// Row tracks `(start, end)` of the watched grids relative to `origin`,
    /// read from the laid-out grid (taffy `DetailedLayoutInfo::Grid`) through
    /// its first recorded item, before the layout engine is released.
    pub(crate) fn rows(
        &self,
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> HashMap<u64, Vec<(f32, f32)>> {
        let mut grids = Vec::new();
        if let Some(b) = &self.blocks {
            b.grids(&mut grids);
        }
        let mut out = HashMap::new();
        for (grid, items) in grids {
            let Some(lid) = items.iter().find_map(|&id| self.seen(id)) else {
                continue;
            };
            let Some((o, _, rows)) = window.parent_grid_tracks(lid) else {
                continue;
            };
            let off = f32::from(o.y - origin.y);
            out.insert(grid, rows.iter().map(|r| (r.0 + off, r.1 + off)).collect());
        }
        out
    }

    /// Block stack watched by the recorder (`for_blocks`).
    pub(crate) fn blocks(&self) -> Option<&BlockBox> {
        self.blocks.as_ref()
    }

    pub(crate) fn for_lines() -> Rc<Self> {
        Rc::new(Self {
            flow: Rc::new(crate::layout::fragment::flow::FlowMeasurements::for_lines()),
            ..Self::default()
        })
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
    }

    /// Layout node recorded for `id` by the last layout.
    pub(crate) fn seen(&self, id: u64) -> Option<LayoutId> {
        self.seen.borrow().get(&id).copied()
    }
}

impl std::fmt::Debug for Watched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watched")
            .field("ids", &self.ids.len())
            .finish()
    }
}

/// Wrap `el` when the build's recorder watches `id` (identity otherwise).
pub(crate) fn wrap(id: u64, el: AnyElement, opts: &RenderOpts) -> AnyElement {
    match &opts.frag.watch {
        Some(rec) if rec.ids.contains(&id) => Watch {
            slot: Slot::Box(id),
            inner: el,
            rec: rec.clone(),
        }
        .hooked(),
        _ => el,
    }
}

/// What a watched node is recorded as.
#[derive(Clone, Copy)]
enum Slot {
    /// Border box of the box with this node id.
    Box(u64),
    /// Part of the table with this key (`table_part`).
    Table(u64, TablePart),
}

struct Watch {
    slot: Slot,
    inner: AnyElement,
    rec: Rc<Watched>,
}

impl Hooks for Watch {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout(window, cx);
        match self.slot {
            Slot::Box(id) => {
                self.rec.seen.borrow_mut().insert(id, lid);
            }
            Slot::Table(key, part) => self.rec.record_part(key, part, lid),
        }
        (lid, ())
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
    }
}
