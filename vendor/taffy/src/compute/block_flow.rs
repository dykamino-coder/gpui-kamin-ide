//! Normalize native block layout into logical axes without replacing its BFC, float or margin rules.
mod geometry;
mod style;
mod tree;

use crate::{
    BlockContext, BlockFlow, CoreStyle, LayoutBlockContainer, LayoutInput, LayoutOutput, NodeId,
};

/// Compute native CSS block layout with optional writing-mode normalization.
pub fn compute_block_layout(
    tree: &mut impl LayoutBlockContainer,
    node: NodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
) -> LayoutOutput {
    let flow = tree.get_block_container_style(node).block_flow();
    if let Some(flow) = flow {
        compute(tree, node, inputs, flow, block_ctx)
    } else {
        super::block::compute_block_layout_normalized(tree, node, inputs, block_ctx)
    }
}

pub(super) fn compute(
    tree: &mut impl LayoutBlockContainer,
    node: NodeId,
    inputs: LayoutInput,
    flow: BlockFlow,
    block_ctx: Option<&mut BlockContext<'_>>,
) -> LayoutOutput {
    let mut logical = self::tree::FlowTree::new(tree, flow);
    let output = super::block::compute_block_layout_normalized(
        &mut logical,
        node,
        flow.input(inputs),
        block_ctx,
    );
    logical.finish(output.size);
    flow.output(output)
}
