//! Expose completed native subtree geometry and baselines for custom flow consumers.
//! Dirty/uncomputed nodes have no completed output; intrinsic probes cannot replace it.

use super::{TaffyError, TaffyResult, TaffyTree};
use crate::{LayoutOutput, NodeId};

impl<NodeContext> TaffyTree<NodeContext> {
    /// Return the most recent full layout result, including content baseline sets.
    /// The result becomes unavailable when this node's native cache is invalidated.
    pub fn computed_layout_output(&self, node: NodeId) -> TaffyResult<Option<LayoutOutput>> {
        self.nodes
            .get(node.into())
            .map(|data| data.cache.performed_layout_output())
            .ok_or(TaffyError::InvalidInputNode(node))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style_helpers::*;
    use crate::{Baselines, MeasureOutput, Rect, Size, Style};

    #[test]
    fn completed_child_baselines_include_the_border_box_inset() {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let node = tree
            .new_leaf_with_context(
                Style {
                    size: Size {
                        width: length(80.0),
                        height: length(40.0),
                    },
                    padding: Rect {
                        top: length(5.0),
                        bottom: length(5.0),
                        left: zero(),
                        right: zero(),
                    },
                    ..Style::DEFAULT
                },
                (),
            )
            .unwrap();
        assert_eq!(tree.computed_layout_output(node).unwrap(), None);
        tree.compute_layout_with_measure(node, Size::MAX_CONTENT, |input, _, _, style| {
            crate::compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |_, _| MeasureOutput {
                    size: Size {
                        width: 80.0,
                        height: 20.0,
                    },
                    baseline: Some(10.0),
                    last_baseline: Some(16.0),
                    baseline_x: None,
                    last_baseline_x: None,
                    baseline_x_from_right: false,
                },
            )
        })
        .unwrap();
        let output = tree.computed_layout_output(node).unwrap().unwrap();
        assert_eq!(output.size, tree.unrounded_layout(node).size);
        assert_eq!(
            output.baselines,
            Baselines {
                first: Some(15.0),
                last: Some(21.0)
            }
        );
        let mut changed = tree.style(node).unwrap().clone();
        changed.size.height = length(50.0);
        tree.set_style(node, changed).unwrap();
        assert_eq!(tree.computed_layout_output(node).unwrap(), None);
    }

    #[test]
    fn removed_node_returns_an_error_instead_of_stale_baselines() {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let node = tree.new_leaf(Style::DEFAULT).unwrap();
        tree.remove(node).unwrap();
        assert!(
            matches!(tree.computed_layout_output(node), Err(TaffyError::InvalidInputNode(id)) if id == node)
        );
    }
}
