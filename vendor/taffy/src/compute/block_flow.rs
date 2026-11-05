//! Normalize native block layout into logical axes without replacing its BFC, float or margin rules.
mod geometry;
mod style;
mod tree;

use crate::{
    BlockContext, BlockFlow, CoreStyle, LayoutBlockContainer, LayoutInput, LayoutOutput, NodeId,
};

/// Compute native CSS block layout with optional writing-mode normalization.
pub async fn compute_block_layout_async(
    tree: &mut impl LayoutBlockContainer,
    node: NodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
    stack: &mut reblessive::Stk,
) -> LayoutOutput {
    let flow = tree.get_block_container_style(node).block_flow();
    // KaminIDE patch: an identity projection needs no recursive FlowTree frames.
    // Orthogonal children and direction overrides still need the adapter's BFC rules.
    if let Some(flow) = flow.filter(|flow| !identity(tree, node, *flow)) {
        compute_async(tree, node, inputs, flow, block_ctx, stack).await
    } else {
        super::block::compute_block_layout_normalized_async(tree, node, inputs, block_ctx, stack)
            .await
    }
}

/// Whether `flow` and every child flow are the default horizontal LTR mapping, so the
/// logical-axis adapter can be bypassed without changing results.
fn identity(tree: &impl LayoutBlockContainer, node: NodeId, flow: BlockFlow) -> bool {
    if flow != BlockFlow::default()
        || tree.get_core_container_style(node).direction() != crate::Direction::Ltr
    {
        return false;
    }
    tree.child_ids(node).all(|child| {
        let style = tree.get_core_container_style(child);
        style.direction() == crate::Direction::Ltr
            && style.block_flow().is_none_or(|child_flow| {
                !child_flow.vertical && !child_flow.block_reverse
            })
    })
}

/// KaminIDE patch: Compute block layout through a logical-axis tree adapter.
// KaminIDE patch: кадр адаптера не должен занимать стек при тождественном отображении.
#[inline(never)]
pub(super) async fn compute_async(
    tree: &mut impl LayoutBlockContainer,
    node: NodeId,
    inputs: LayoutInput,
    flow: BlockFlow,
    block_ctx: Option<&mut BlockContext<'_>>,
    stack: &mut reblessive::Stk,
) -> LayoutOutput {
    let mut logical = self::tree::FlowTree::new(tree, flow);
    let output = super::block::compute_block_layout_normalized_async(
        &mut logical,
        node,
        flow.input(inputs),
        block_ctx,
        stack,
    )
    .await;
    logical.finish(output.size);
    flow.output(output)
}
