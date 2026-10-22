//! Прозрачная обёртка `Watch`, сообщающая узел раскладки наблюдаемой коробки. Решателю
//! пагинации нужны разложенные border-box кандидатов разрыва внутри копии. Пагинируемая
//! копия строится с рекордером `Watched` в `RenderOpts` (`FragCx::watch`); `render::element`
//! оборачивает элемент, чей id рекордер наблюдает, а обёртка записывает внутренний узел
//! раскладки и отдаёт его как свой — taffy видит то же дерево. Узлы читает `seen`, `reset`
//! забывает их перед замером. Рекордер блочной стопки (`for_blocks`) наблюдает коробки
//! `flow::blocks::BlockBox` главной копии ребёнка стопки и после её раскладки отдаёт точки
//! разрыва по разложенной геометрии (`breaks`).

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::fragment::flow::blocks::{BlockBox, BlockBreaks, MeasuredBreaks};
use crate::layout::fragment::types::StackAxis;
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Point, Size, Window};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Break candidates of one paginated child and the layout nodes its last
/// measuring layout gave them. Owned by the child's `Paginate`, shared with
/// the wrappers of its rebuilt copies.
#[derive(Default)]
pub(crate) struct Watched {
    pub(crate) flow: Rc<crate::layout::fragment::flow::FlowMeasurements>,
    ids: HashSet<u64>,
    seen: RefCell<HashMap<u64, LayoutId>>,
    /// Block stack of the watched copy whose boundaries are read after layout.
    blocks: Option<BlockBox>,
}

impl Watched {
    pub(crate) fn new(ids: &[u64]) -> Rc<Self> {
        Rc::new(Watched {
            ids: ids.iter().copied().collect(),
            seen: RefCell::default(),
            flow: Rc::default(),
            blocks: None,
        })
    }

    /// Recorder of a stack child's main copy: measured line edges plus the
    /// border boxes of its block stack `blocks`.
    pub(crate) fn for_blocks(blocks: BlockBox) -> Rc<Self> {
        Rc::new(Watched {
            ids: blocks.ids().into_iter().collect(),
            seen: RefCell::default(),
            flow: Rc::new(crate::layout::fragment::flow::FlowMeasurements::for_lines()),
            blocks: Some(blocks),
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
            (Some(b), StackAxis::Horizontal) => {
                let boxes: HashMap<u64, (f32, f32)> = self
                    .seen
                    .borrow()
                    .iter()
                    .map(|(&id, &lid)| {
                        let top = f32::from(window.layout_origin_unrounded(lid).y - origin.y);
                        let h = f32::from(window.layout_size_unrounded(lid).height);
                        (id, (top, top + h))
                    })
                    .collect();
                b.breaks(end, &|id| boxes.get(&id).copied())
            }
            _ => BlockBreaks::default(),
        };
        blocks.finish(lines, end)
    }

    pub(crate) fn for_lines() -> Rc<Self> {
        Rc::new(Self {
            flow: Rc::new(crate::layout::fragment::flow::FlowMeasurements::for_lines()),
            ..Self::default()
        })
    }

    /// Forget recorded nodes before a measuring layout.
    pub(crate) fn reset(&self) {
        self.seen.borrow_mut().clear();
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
            id,
            inner: el,
            rec: rec.clone(),
        }
        .hooked(),
        _ => el,
    }
}

struct Watch {
    id: u64,
    inner: AnyElement,
    rec: Rc<Watched>,
}

impl Hooks for Watch {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout(window, cx);
        self.rec.seen.borrow_mut().insert(self.id, lid);
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
