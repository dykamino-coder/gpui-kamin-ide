//! Ограничения одного вложенного ряда колонок из его реального внешнего Space.

use super::{Kind, Space};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NestedRow {
    pub(crate) outer_block: f32,
    pub(crate) offset: f32,
    /// Тип ближайшего внешнего контекста фрагментации.
    pub(crate) outer_paged: bool,
}

impl NestedRow {
    pub(crate) fn from_space(outer: Space, offset: f32) -> Option<Self> {
        let outer_block = outer.block_size?;
        if !outer_block.is_finite() || outer_block < 0. || !offset.is_finite() {
            return None;
        }
        Some(Self {
            outer_block,
            offset: offset.max(0.),
            outer_paged: matches!(outer.kind, Kind::Pages),
        })
    }

    pub(crate) fn remaining(self) -> f32 {
        (self.outer_block - self.offset).max(0.)
    }
}
