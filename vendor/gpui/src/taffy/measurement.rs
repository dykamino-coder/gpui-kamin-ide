//! Measure a copied native subtree while the Window layout engine is borrowed by a parent.
//! Styles and cache state are independent; actual element measurement callbacks are shared.

use super::{AvailableSpace, EXPECT_MESSAGE, LayoutId, NodeContext, TaffyLayoutEngine};
use crate::{App, Pixels, Size, Window, size};
use taffy::{LayoutInput, LayoutOutput, NodeId, Style, TaffyTree};

#[path = "visible_baselines.rs"]
mod visible_baselines;

#[path = "physical_measurement.rs"]
mod physical_measurement;

#[path = "visible_baselines_x.rs"]
mod visible_baselines_x;

/// Independently cached measurement of an element subtree in the current layout phase.
/// Consume it before the window begins a new frame; element callbacks are shared.
pub struct LayoutMeasurement {
    tree: TaffyTree<NodeContext>,
    root: NodeId,
    scale_factor: f32,
}

impl TaffyLayoutEngine {
    pub fn snapshot_measurement(&self, root: LayoutId, scale_factor: f32) -> LayoutMeasurement {
        let mut tree = TaffyTree::new();
        tree.disable_rounding();
        // The copied styles carry the engine tree's `calc()` handles.
        tree.share_calc_registry(&self.taffy);
        let root = copy_subtree(&self.taffy, &mut tree, root.into());
        LayoutMeasurement {
            tree,
            root,
            scale_factor,
        }
    }
}

// KaminIDE patch: postorder copying keeps document depth off the native stack.
fn copy_subtree(
    source: &TaffyTree<NodeContext>,
    target: &mut TaffyTree<NodeContext>,
    node: NodeId,
) -> NodeId {
    struct Frame {
        node: NodeId,
        children: std::vec::IntoIter<NodeId>,
        copies: Vec<NodeId>,
    }
    let frame = |node| {
        let children = source.children(node).expect(EXPECT_MESSAGE);
        Frame {
            node,
            copies: Vec::with_capacity(children.len()),
            children: children.into_iter(),
        }
    };
    let mut stack = vec![frame(node)];
    loop {
        if let Some(child) = stack.last_mut().unwrap().children.next() {
            stack.push(frame(child));
            continue;
        }
        let finished = stack.pop().unwrap();
        let copy = target
            .new_with_children(
                source.shared_style(finished.node).expect(EXPECT_MESSAGE),
                &finished.copies,
            )
            .expect(EXPECT_MESSAGE);
        if let Some(context) = source.get_node_context(finished.node) {
            target
                .set_node_context(copy, Some(context.clone()))
                .expect(EXPECT_MESSAGE);
        }
        match stack.last_mut() {
            Some(parent) => parent.copies.push(copy),
            None => return copy,
        }
    }
}

impl LayoutMeasurement {
    /// Cap the margin-box inline size of the root's `index`-th child in this
    /// copy only (see [`TaffyLayoutEngine::cap_child_outer_width`]).
    pub fn cap_child_outer_width(&mut self, index: usize, width: Pixels) -> bool {
        super::cap_child_outer_width(
            &mut self.tree,
            self.root,
            index,
            width.0 * self.scale_factor,
        )
    }

    /// Measure logical CSS size and content baseline sets without paint rounding.
    /// Callbacks have the same restrictions as ordinary native layout callbacks.
    // KaminIDE patch: native measure callbacks are stack-safe values; enter the
    // stack-safe context like `TaffyLayoutEngine::compute_layout`.
    #[cfg_attr(feature = "stacker", stacksafe::stacksafe)]
    pub fn measure(
        &mut self,
        available: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        reblessive::Stack::new()
            .enter(|stack| self.measure_async(available, window, cx, stack))
            .finish()
    }
}

fn to_native(value: AvailableSpace, scale: f32) -> taffy::AvailableSpace {
    match value {
        AvailableSpace::Definite(value) => taffy::AvailableSpace::Definite(value.0 * scale),
        AvailableSpace::MinContent => taffy::AvailableSpace::MinContent,
        AvailableSpace::MaxContent => taffy::AvailableSpace::MaxContent,
    }
}

pub(super) fn leaf(
    input: LayoutInput,
    mut context: Option<&mut NodeContext>,
    style: &Style,
    scale: f32,
    window: &mut Window,
    cx: &mut App,
) -> LayoutOutput {
    reblessive::Stack::new()
        .enter(|stack| leaf_async(input, context, style, scale, window, cx, stack))
        .finish()
}

#[cfg(test)]
#[path = "measurement_tests.rs"]
mod tests;

mod leaf_async;
pub(super) use leaf_async::leaf_async;

mod continuation;
