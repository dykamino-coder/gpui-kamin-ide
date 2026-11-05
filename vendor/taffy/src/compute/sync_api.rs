//! Синхронные точки входа запускают тот же обход на явном стеке.

use crate::*;

/// KaminIDE patch: Drive the bounded native-stack layout traversal.
pub fn compute_oof_layout(
    tree: &mut impl LayoutContainingBlock,
    node_id: NodeId,
    output: &mut LayoutOutput,
) {
    reblessive::Stack::new()
        .enter(|stack| super::oof::compute_oof_layout_async(tree, node_id, output, stack))
        .finish()
}

/// KaminIDE patch: Drive the bounded native-stack layout traversal.
pub fn compute_oof_layout_for_area(
    tree: &mut impl LayoutContainingBlock,
    geometry_owner: NodeId,
    candidates: &[OofCandidate],
    area: OofPositioningArea,
    direction: Direction,
    claims: ContainingBlockClaims,
) -> OofLayoutResult {
    reblessive::Stack::new()
        .enter(|stack| {
            super::oof::compute_oof_layout_for_area_async(
                tree,
                geometry_owner,
                candidates,
                area,
                direction,
                claims,
                stack,
            )
        })
        .finish()
}

#[cfg(feature = "block_layout")]
/// KaminIDE patch: Drive the bounded native-stack layout traversal.
pub fn compute_block_layout(
    tree: &mut impl LayoutBlockContainer,
    node: NodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
) -> LayoutOutput {
    reblessive::Stack::new()
        .enter(|stack| {
            super::block_flow::compute_block_layout_async(tree, node, inputs, block_ctx, stack)
        })
        .finish()
}

#[cfg(feature = "flexbox")]
/// KaminIDE patch: Drive the bounded native-stack layout traversal.
pub fn compute_flexbox_layout(
    tree: &mut impl LayoutFlexboxContainer,
    node: NodeId,
    inputs: LayoutInput,
) -> LayoutOutput {
    reblessive::Stack::new()
        .enter(|stack| super::flexbox::compute_flexbox_layout_async(tree, node, inputs, stack))
        .finish()
}

#[cfg(feature = "grid")]
/// KaminIDE patch: Drive the bounded native-stack layout traversal.
pub fn compute_grid_layout<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    node: NodeId,
    inputs: LayoutInput,
) -> LayoutOutput {
    reblessive::Stack::new()
        .enter(|stack| super::grid::compute_grid_layout_async(tree, node, inputs, stack))
        .finish()
}

/// KaminIDE patch: Drive the bounded native-stack layout traversal.
pub fn compute_root_layout(
    tree: &mut (impl crate::tree::LayoutContainingBlock + CacheTree),
    root: NodeId,
    available_space: Size<AvailableSpace>,
) {
    reblessive::Stack::new()
        .enter(|stack| super::compute_root_layout_async(tree, root, available_space, stack))
        .finish()
}

/// KaminIDE patch: Drive the bounded native-stack layout traversal.
pub fn compute_hidden_layout(
    tree: &mut (impl LayoutPartialTree + CacheTree),
    node: NodeId,
) -> LayoutOutput {
    reblessive::Stack::new()
        .enter(|stack| super::compute_hidden_layout_async(tree, node, stack))
        .finish()
}
