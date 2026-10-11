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
        let root = copy_subtree(&self.taffy, &mut tree, root.into());
        LayoutMeasurement {
            tree,
            root,
            scale_factor,
        }
    }
}

#[stacksafe::stacksafe]
fn copy_subtree(
    source: &TaffyTree<NodeContext>,
    target: &mut TaffyTree<NodeContext>,
    node: NodeId,
) -> NodeId {
    let children: Vec<_> = source
        .children(node)
        .expect(EXPECT_MESSAGE)
        .into_iter()
        .map(|child| copy_subtree(source, target, child))
        .collect();
    let copy = target
        .new_with_children(source.style(node).expect(EXPECT_MESSAGE).clone(), &children)
        .expect(EXPECT_MESSAGE);
    if let Some(context) = source.get_node_context(node) {
        target
            .set_node_context(copy, Some(context.clone()))
            .expect(EXPECT_MESSAGE);
    }
    copy
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
    pub fn measure(
        &mut self,
        available: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        let scale = self.scale_factor;
        let available = taffy::Size {
            width: to_native(available.width, scale),
            height: to_native(available.height, scale),
        };
        self.tree
            .compute_layout_with_measure(self.root, available, |input, _, context, style| {
                leaf(input, context, style, scale, window, cx)
            })
            .expect(EXPECT_MESSAGE);
        let output = self
            .tree
            .computed_layout_output(self.root)
            .expect(EXPECT_MESSAGE)
            .expect("the subtree completed full layout");
        (
            size(
                Pixels(output.size.width / scale),
                Pixels(output.size.height / scale),
            ),
            output.baselines.first.map(|value| Pixels(value / scale)),
            output.baselines.last.map(|value| Pixels(value / scale)),
        )
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
    let performed = matches!(input.run_mode, taffy::RunMode::PerformLayout);
    if performed {
        if let Some(context) = context.as_deref_mut() {
            context.layout_lines = None;
            context.layout_lines_x = None;
            context.layout_lines_x_from_right = false;
        }
    }
    taffy::compute_leaf_layout(input, style, taffy::tree::calc_value, |known, available| {
        crate::frame_perf::LAYOUT_MEASURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _timer = super::MeasureTimer(std::time::Instant::now());
        let Some(context) = context else {
            return taffy::Size::default().into();
        };
        let known = Size {
            width: known.width.map(|value| Pixels(value / scale)),
            height: known.height.map(|value| Pixels(value / scale)),
        };
        let from_native = |value| match value {
            taffy::AvailableSpace::Definite(value) => {
                AvailableSpace::Definite(Pixels(value / scale))
            }
            taffy::AvailableSpace::MinContent => AvailableSpace::MinContent,
            taffy::AvailableSpace::MaxContent => AvailableSpace::MaxContent,
        };
        let available = size(from_native(available.width), from_native(available.height));
        let measured = (context.measure.borrow_mut())(known, available, window, cx);
        let output = measured.to_native(scale);
        // Intrinsic probes must not replace metadata belonging to a cached full layout.
        if performed {
            context.layout_lines = measured.lines_y;
            context.layout_lines_x = measured.lines_x;
            context.layout_lines_x_from_right = measured.x_from_right;
        }
        output
    })
}

#[cfg(test)]
#[path = "measurement_tests.rs"]
mod tests;
