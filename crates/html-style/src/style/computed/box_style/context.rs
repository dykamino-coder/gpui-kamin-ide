//! Производитель фактов родителя и транспорт существующего layout-made моста.

use crate::box_facts::BoxContext;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Align, BoxStyle, Display as D};

pub(super) fn child_of(parent: &BoxStyle) -> BoxContext<WritingMode, Align> {
    let parent_grid = match parent.reset_box_layout.display {
        Some(D::Grid | D::InlineGrid) => match (parent.wm().vertical(), parent.wm().block_rl()) {
            (false, _) => 1,
            (true, false) => 2,
            (true, true) => 3,
        },
        _ => 0,
    };
    BoxContext {
        parent_grid,
        parent_grid_align_items: parent.reset_flex.align_items,
        parent_flex_grid: matches!(
            parent.reset_box_layout.display,
            Some(D::Flex | D::InlineFlex | D::Grid | D::InlineGrid | D::GridLanes)
        ),
        parent_lanes: parent.reset_box_layout.display == Some(D::GridLanes),
        cb_wm: parent.wm().line(),
        applicability: parent.applicability_inputs(),
    }
}

#[cfg(test)]
mod tests;
