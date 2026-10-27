//! Исходные координаты OOF: неизменяемый результат normal layout до фрагментации.
//! CB, static rect и ограничения принадлежат одному завершённому проходу.

use crate::layout::fragment::geometry::Node;
use gpui::{Bounds, LayoutSpace, Pixels, Point};
use std::collections::HashMap;

pub(crate) mod fragmentation;
pub(crate) mod layout;
pub(crate) mod record;
pub(crate) mod static_rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dependencies {
    pub(crate) containing_block: LayoutSpace,
    pub(crate) positioned: LayoutSpace,
    pub(crate) static_position: Option<LayoutSpace>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PositionedMetrics {
    pub(crate) cb_id: u64,
    pub(crate) cb_fragment_id: Node,
    pub(crate) original_cb_rect: Bounds<Pixels>,
    pub(crate) static_rect: Option<Bounds<Pixels>>,
    pub(crate) resolved_constraints: LayoutSpace,
    pub(crate) normal_offset: Point<Pixels>,
    pub(crate) normal_size: gpui::Size<Pixels>,
    pub(crate) dependencies: Dependencies,
}

impl PositionedMetrics {
    pub(crate) fn normal_rect(&self) -> Bounds<Pixels> {
        debug_assert_eq!(self.resolved_constraints, self.dependencies.positioned);
        debug_assert_eq!(
            self.static_rect.is_some(),
            self.dependencies.static_position.is_some()
        );
        Bounds {
            origin: self.original_cb_rect.origin + self.normal_offset,
            size: self.normal_size,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Snapshot {
    pub(crate) positioned: HashMap<u64, PositionedMetrics>,
    pub(crate) containing_blocks: HashMap<u64, (Node, LayoutSpace)>,
    pub(crate) flows: Vec<fragmentation::Source>,
    pub(crate) families: HashMap<u64, u64>,
    pub(crate) normal: HashMap<u64, Bounds<Pixels>>,
    /// Accumulated used relative displacement of each painted box (zero omitted):
    /// the source paints there, the normal geometry above excludes it.
    pub(crate) relative: HashMap<u64, Point<Pixels>>,
    pub(crate) cloned_boxes: HashMap<u64, (crate::layout::fragment::types::StackAxis, f32, f32)>,
    pub(crate) native: Option<std::rc::Rc<layout::Native>>,
    /// Fragmentation host whose children the source lays out (0: a page area).
    pub(crate) host: u64,
    /// Fragmentation axis the source laid its flows out along.
    pub(crate) axis: Option<crate::layout::fragment::types::StackAxis>,
}

impl Snapshot {
    /// The native source paints only when an absolute or fixed flow continues;
    /// floats are recorded for geometry but stay in their owner's normal flow.
    pub(crate) fn native_oof(&self) -> Option<&std::rc::Rc<layout::Native>> {
        self.native
            .as_ref()
            .filter(|_| self.flows.iter().any(|flow| flow.native))
    }

    /// The native source of a stack fragmenting along `vertical`: a stack laid
    /// out on another axis (the vertical `direction: rtl` column path) cannot
    /// route the source's continuations, which its own copies keep painting.
    pub(crate) fn native_along(&self, vertical: bool) -> Option<&std::rc::Rc<layout::Native>> {
        self.native_oof()
            .filter(|_| self.axis.is_some_and(|axis| axis.is_vertical() == vertical))
    }

    /// Outermost roots of the continued flows: the boxes whose legacy copies
    /// replay their own records.
    pub(crate) fn continued_roots(&self) -> std::rc::Rc<[u64]> {
        let mut roots: Vec<u64> = self
            .flows
            .iter()
            .filter(|flow| flow.native)
            .map(|flow| flow.root)
            .collect();
        roots.sort_unstable();
        roots.dedup();
        roots.into()
    }

    pub(crate) fn get(&self, id: u64) -> Option<&PositionedMetrics> {
        let metrics = self.positioned.get(&id)?;
        let &(node, space) = self.containing_blocks.get(&metrics.cb_id)?;
        (node == metrics.cb_fragment_id && space == metrics.dependencies.containing_block)
            .then_some(metrics)
    }
}
