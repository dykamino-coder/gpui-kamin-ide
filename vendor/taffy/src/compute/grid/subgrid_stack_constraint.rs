//! Preserve row-lanes fit-content width before standalone tracks contribute.
use super::super::lanes_stack_measure;
use super::super::types::GridItem;
use crate::geometry::Size;
use crate::tree::LayoutPartialTreeExt;
use crate::util::{MaybeResolve, ResolveOrZero};
use crate::{CoreStyle, LayoutGridContainer, SUBGRID_ROWS};

/// KaminIDE patch: Resolve the subgrid root's stacking-axis content constraint.
pub(super) fn root_content(
    tree: &mut impl LayoutGridContainer,
    item: &GridItem,
    parent: Size<Option<f32>>,
    mask: u8,
) -> Size<Option<f32>> {
    if mask != SUBGRID_ROWS || !item.size.width.is_auto() {
        return Size::NONE;
    }
    let style = tree.get_grid_container_style(item.node);
    if style.block_flow().is_some_and(|flow| flow.vertical) {
        return Size::NONE;
    }
    drop(style);
    let Some(available) = parent.width else {
        return Size::NONE;
    };
    let margins = item
        .margin
        .resolve_or_zero(parent.width, |v, b| tree.calc(v, b));
    let height = item
        .size
        .height
        .maybe_resolve(parent.height, |v, b| tree.calc(v, b));
    let max = lanes_stack_measure::width(tree, item.node, height, parent, height.unwrap_or(0.0));
    let room = (available - margins.left - margins.right).max(0.0);
    let fit = if max > room {
        let min =
            lanes_stack_measure::min_width(tree, item.node, height, parent, height.unwrap_or(0.0));
        max.min(min.max(room))
    } else {
        max
    };
    let padding = item
        .padding
        .resolve_or_zero(parent.width, |v, b| tree.calc(v, b));
    let border = item
        .border
        .resolve_or_zero(parent.width, |v, b| tree.calc(v, b));
    let edges = (padding + border).sum_axes();
    Size {
        width: Some((fit - edges.width).max(0.0)),
        height: None,
    }
}

#[cfg(test)]
#[path = "subgrid_stack_constraint_tests.rs"]
mod tests;
