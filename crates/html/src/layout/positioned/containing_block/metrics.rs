//! Исходные координаты OOF: неизменяемый результат normal layout до фрагментации.
//! CB, static rect и ограничения принадлежат одному завершённому проходу.

use crate::layout::fragment::geometry::Node;
use gpui::{Bounds, LayoutSpace, Pixels, Point};
use std::collections::HashMap;

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
}

impl Snapshot {
    pub(crate) fn get(&self, id: u64) -> Option<&PositionedMetrics> {
        let metrics = self.positioned.get(&id)?;
        let &(node, space) = self.containing_blocks.get(&metrics.cb_id)?;
        (node == metrics.cb_fragment_id && space == metrics.dependencies.containing_block)
            .then_some(metrics)
    }
}
