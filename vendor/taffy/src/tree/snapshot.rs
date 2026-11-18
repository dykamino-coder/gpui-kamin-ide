//! Copy a subtree's completed results without retaining its source tree.

use super::{TaffyResult, TaffyTree};
#[cfg(feature = "flexbox")]
use crate::tree::DetailedLayoutInfo;
use crate::tree::NodeId;
use crate::util::sys::{Map, Vec};

impl<NodeContext: Clone> TaffyTree<NodeContext> {
    /// Copy one subtree, including its independent native layout cache.
    ///
    /// Measurement contexts are cloned, not evaluated. Callers must keep their
    /// measurement dependencies unchanged for the lifetime of the snapshot.
    /// Cached node references are translated to the new tree's identities.
    pub fn snapshot_subtree(&self, root: NodeId) -> TaffyResult<(Self, NodeId)> {
        self.style(root)?;
        let mut nodes = Vec::new();
        nodes.push(root);
        let mut cursor = 0;
        while cursor < nodes.len() {
            nodes.extend_from_slice(self.children_slice(nodes[cursor]));
            cursor += 1;
        }
        let mut target = Self::with_capacity(nodes.len());
        target.config = self.config;
        #[cfg(feature = "std")]
        target.share_calc_registry(self);
        let mut ids = Map::default();
        for &node in &nodes {
            let copy = target.new_leaf(self.shared_style(node)?)?;
            ids.insert(u64::from(node), copy);
        }
        for &node in &nodes {
            let copy = ids[&u64::from(node)];
            let mut data = self.nodes[node.into()].clone();
            data.identity_pinned = false;
            for child in &mut data.hoisted_children {
                *child = ids[&u64::from(*child)];
            }
            data.cache.remap_nodes(|id| ids[&u64::from(id)]);
            #[cfg(feature = "flexbox")]
            if let DetailedLayoutInfo::Flex(info) = &mut data.detailed_layout_info {
                for line in &mut info.lines {
                    for item in &mut line.items {
                        item.node = ids[&u64::from(item.node)];
                    }
                }
            }
            target.nodes[copy.into()] = data;
            target.children[copy.into()] = self
                .children_slice(node)
                .iter()
                .map(|id| ids[&u64::from(*id)])
                .collect();
            target.parents[copy.into()] = if node == root {
                None
            } else {
                self.parent(node).map(|id| ids[&u64::from(id)])
            };
            if let Some(context) = self.get_node_context(node) {
                target
                    .node_context_data
                    .insert(copy.into(), context.clone());
            }
        }
        Ok((target, ids[&u64::from(root)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style_helpers::*;
    use crate::{Baselines, MeasureOutput, Size, Style};

    fn measure(tree: &mut TaffyTree<()>, root: NodeId, width: f32) -> usize {
        let mut calls = 0;
        tree.compute_layout_with_measure(
            root,
            Size {
                width: crate::AvailableSpace::Definite(width),
                height: crate::AvailableSpace::MaxContent,
            },
            |input, _, _, style| {
                crate::compute_leaf_layout(
                    input,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        calls += 1;
                        let width = known
                            .width
                            .unwrap_or_else(|| available.width.into_option().unwrap_or(100.0));
                        MeasureOutput {
                            size: Size {
                                width,
                                height: 20.0,
                            },
                            baseline: Some(12.0),
                            last_baseline: Some(16.0),
                            baseline_x: None,
                            last_baseline_x: None,
                            baseline_x_from_right: false,
                        }
                    },
                )
            },
        )
        .unwrap();
        calls
    }

    #[test]
    fn completed_snapshot_reuses_result_and_descendant_geometry() {
        let mut source = TaffyTree::new();
        source.disable_rounding();
        let leaf = source.new_leaf_with_context(Style::DEFAULT, ()).unwrap();
        let root = source.new_with_children(Style::DEFAULT, &[leaf]).unwrap();
        assert!(measure(&mut source, root, 80.0) > 0);
        let original = *source.unrounded_layout(leaf);
        let output = source.computed_layout_output(root).unwrap();
        let (mut copy, copied_root) = source.snapshot_subtree(root).unwrap();
        let copied_leaf = copy.children_slice(copied_root)[0];
        assert_ne!(
            copied_root, root,
            "The snapshot must translate node identities."
        );
        assert_eq!(copy.computed_layout_output(copied_root).unwrap(), output);
        assert_eq!(*copy.unrounded_layout(copied_leaf), original);
        source.clear();
        assert_eq!(measure(&mut copy, copied_root, 80.0), 0);
        assert_eq!(*copy.unrounded_layout(copied_leaf), original);
        assert_eq!(
            copy.computed_layout_output(copied_leaf)
                .unwrap()
                .unwrap()
                .baselines,
            Baselines {
                first: Some(12.0),
                last: Some(16.0)
            }
        );
        // The root must miss at a new width; an unchanged intrinsic leaf may still hit.
        measure(&mut copy, copied_root, 120.0);
        assert_eq!(copy.unrounded_layout(copied_root).size.width, 100.0);
    }

    #[test]
    fn snapshot_style_invalidation_is_independent() {
        let mut source = TaffyTree::new();
        let root = source.new_leaf_with_context(Style::DEFAULT, ()).unwrap();
        measure(&mut source, root, 80.0);
        let original = source.computed_layout_output(root).unwrap();
        let (mut copy, copied_root) = source.snapshot_subtree(root).unwrap();
        copy.set_style(
            copied_root,
            Style {
                size: Size {
                    width: length(30.0),
                    height: auto(),
                },
                ..Style::DEFAULT
            },
        )
        .unwrap();
        assert!(copy.computed_layout_output(copied_root).unwrap().is_none());
        assert_eq!(source.computed_layout_output(root).unwrap(), original);
        measure(&mut copy, copied_root, 80.0);
        assert_eq!(copy.unrounded_layout(copied_root).size.width, 30.0);
        source.mark_dirty(root).unwrap();
        let (copy, copied_root) = source.snapshot_subtree(root).unwrap();
        assert!(copy.computed_layout_output(copied_root).unwrap().is_none());
    }

    #[cfg(feature = "flexbox")]
    #[test]
    fn snapshot_translates_completed_flex_item_ids() {
        let mut source: TaffyTree<()> = TaffyTree::new();
        let leaf = source
            .new_leaf(Style {
                size: Size {
                    width: length(20.0),
                    height: length(10.0),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = source.new_with_children(Style::DEFAULT, &[leaf]).unwrap();
        source.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let (copy, copied_root) = source.snapshot_subtree(root).unwrap();
        let DetailedLayoutInfo::Flex(info) = copy.detailed_layout_info(copied_root) else {
            panic!("Expected completed flex geometry");
        };
        assert_eq!(
            info.lines[0].items[0].node,
            copy.children_slice(copied_root)[0]
        );
    }

    #[test]
    fn snapshot_translates_pending_and_hoisted_out_of_flow_boxes() {
        let mut source: TaffyTree<()> = TaffyTree::new();
        let fixed = source
            .new_leaf(Style {
                position: crate::Position::Fixed,
                size: Size {
                    width: length(12.0),
                    height: length(8.0),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let wrapper = source.new_with_children(Style::DEFAULT, &[fixed]).unwrap();
        let root = source
            .new_with_children(Style::DEFAULT, &[wrapper])
            .unwrap();
        source.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let (mut copy, copied_wrapper) = source.snapshot_subtree(wrapper).unwrap();
        let copied_fixed = copy.children_slice(copied_wrapper)[0];
        let pending = copy
            .computed_layout_output(copied_wrapper)
            .unwrap()
            .unwrap();
        assert_eq!(pending.oof_candidates.as_slice()[0].node, copied_fixed);
        source.clear();
        copy.compute_layout(copied_wrapper, Size::MAX_CONTENT)
            .unwrap();
        assert_eq!(
            copy.hoisted_children(copied_wrapper).unwrap(),
            &[copied_fixed]
        );
        assert_eq!(
            copy.unrounded_layout(copied_fixed).size,
            Size {
                width: 12.0,
                height: 8.0
            }
        );
        let (copy, root) = copy.snapshot_subtree(copied_wrapper).unwrap();
        assert_eq!(
            copy.hoisted_children(root).unwrap(),
            copy.children_slice(root)
        );
    }
}
