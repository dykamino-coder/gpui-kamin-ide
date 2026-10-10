//! Adapt immediate-child layout requests and defer coordinate projection until block size is known.
use super::style::Logical;
use crate::util::sys::Vec;
use crate::{
    BlockContext, BlockFlow, CoreStyle, Layout, LayoutBlockContainer, LayoutInput, LayoutOutput,
    LayoutPartialTree, NodeId, Size, TraversePartialTree,
};

pub(super) struct FlowTree<'a, T> {
    tree: &'a mut T,
    flow: BlockFlow,
    pending: Vec<(NodeId, Layout)>,
}

impl<'a, T: LayoutBlockContainer> FlowTree<'a, T> {
    pub(super) fn new(tree: &'a mut T, flow: BlockFlow) -> Self {
        Self {
            tree,
            flow,
            pending: Vec::new(),
        }
    }

    pub(super) fn finish(&mut self, size: Size<f32>) {
        for (node, layout) in self.pending.drain(..) {
            self.tree
                .set_unrounded_layout(node, &self.flow.layout(layout, size));
        }
    }

    fn parallel(&self, node: NodeId) -> bool {
        self.tree
            .get_core_container_style(node)
            .block_flow()
            .map_or(!self.flow.vertical, |flow| {
                flow.vertical == self.flow.vertical && flow.block_reverse == self.flow.block_reverse
            })
    }
}

impl<T: LayoutBlockContainer> TraversePartialTree for FlowTree<'_, T> {
    type ChildIter<'a>
        = T::ChildIter<'a>
    where
        Self: 'a;
    fn child_ids(&self, node: NodeId) -> Self::ChildIter<'_> {
        self.tree.child_ids(node)
    }
    fn child_count(&self, node: NodeId) -> usize {
        self.tree.child_count(node)
    }
    fn get_child_id(&self, node: NodeId, index: usize) -> NodeId {
        self.tree.get_child_id(node, index)
    }
}

impl<T: LayoutBlockContainer> LayoutPartialTree for FlowTree<'_, T> {
    type CustomIdent = T::CustomIdent;
    type CoreContainerStyle<'a>
        = Logical<T::CoreContainerStyle<'a>>
    where
        Self: 'a;
    fn get_core_container_style(&self, node: NodeId) -> Self::CoreContainerStyle<'_> {
        Logical {
            style: self.tree.get_core_container_style(node),
            flow: self.flow,
        }
    }
    fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
        self.tree.resolve_calc_value(val, basis)
    }
    fn set_unrounded_layout(&mut self, node: NodeId, value: &Layout) {
        if let Some((_, layout)) = self.pending.iter_mut().find(|(id, _)| *id == node) {
            *layout = *value;
        } else {
            self.pending.push((node, *value));
        }
    }
    fn compute_child_layout(&mut self, node: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let output = self
            .tree
            .compute_child_layout(node, self.flow.input(inputs));
        self.flow.logical_output(output)
    }
}

impl<T: LayoutBlockContainer> LayoutBlockContainer for FlowTree<'_, T> {
    type BlockContainerStyle<'a>
        = Logical<T::BlockContainerStyle<'a>>
    where
        Self: 'a;
    type BlockItemStyle<'a>
        = Logical<T::BlockItemStyle<'a>>
    where
        Self: 'a;
    fn get_block_container_style(&self, node: NodeId) -> Self::BlockContainerStyle<'_> {
        Logical {
            style: self.tree.get_block_container_style(node),
            flow: self.flow,
        }
    }
    fn get_block_child_style(&self, node: NodeId) -> Self::BlockItemStyle<'_> {
        Logical {
            style: self.tree.get_block_child_style(node),
            flow: self.flow,
        }
    }
    fn compute_block_child_layout(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        context: Option<&mut BlockContext<'_>>,
    ) -> LayoutOutput {
        let parallel = self.parallel(node);
        let output = self.tree.compute_block_child_layout(
            node,
            self.flow.input(inputs),
            if parallel { context } else { None },
        );
        self.flow.logical_output(output)
    }
}
