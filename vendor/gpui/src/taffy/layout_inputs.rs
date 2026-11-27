//! KaminIDE patch: which boxes of the frame tree a pass laid out with the
//! inputs of the previous pass (`crate::LayoutInputCounts`, the layout journal).
//!
//! A box is named by its path: the order of its pass's layout roots and its
//! child index at every level. Between passes over the same document the
//! paths name the same boxes whatever nodes the retained tree gave them, so the
//! count does not depend on what the tree kept. A box's inputs are the digest
//! of its last full layout (taffy `performed_input_key`); a box without one
//! (hidden) is neither counted nor breaks its parent's subtree. The walk is
//! iterative: documents are deeper than the native stack allows recursion.

use crate::LayoutInputCounts;
use collections::{FxHashMap, FxHashSet};
use taffy::{TaffyTree, TraversePartialTree as _, tree::NodeId};

/// The inputs of the noted boxes of a pass, by path; kept for the next pass.
#[derive(Default)]
pub(super) struct Noted {
    /// Roots laid out in this pass, in their order (`compute_layout`).
    pub(super) roots: Vec<NodeId>,
    last: FxHashMap<u64, u64>,
}

fn step(path: u64, index: u64) -> u64 {
    (path ^ index.wrapping_add(0x9e37_79b9_7f4a_7c15))
        .wrapping_mul(0xbf58_476d_1ce4_e5b9)
        .rotate_left(31)
}

impl Noted {
    /// Count the boxes of this pass's roots against the last pass and keep
    /// their inputs for the next one.
    pub(super) fn pass<T>(&mut self, tree: &TaffyTree<T>) -> LayoutInputCounts {
        let mut counts = LayoutInputCounts {
            passes: 1,
            ..Default::default()
        };
        let mut now = FxHashMap::with_capacity_and_hasher(self.last.len(), Default::default());
        let mut seen = FxHashSet::default();
        // (node, path, next child, subtree kept so far, kept child subtrees)
        let mut stack: Vec<(NodeId, u64, usize, bool, u64)> = Vec::new();
        for (order, &root) in self.roots.iter().enumerate() {
            if !seen.insert(root) {
                continue;
            }
            stack.push((root, step(0, order as u64), 0, true, 0));
            while let Some(top) = stack.last_mut() {
                let (node, path, next) = (top.0, top.1, top.2);
                if next < tree.child_count(node) {
                    top.2 += 1;
                    let child = tree.get_child_id(node, next);
                    if seen.insert(child) {
                        stack.push((child, step(path, next as u64), 0, true, 0));
                    }
                    continue;
                }
                let (_, _, _, kids_kept, kept_kids) = stack.pop().expect("top of the walk");
                let same = match tree.performed_input_key(node) {
                    Some(key) => {
                        counts.boxes += 1;
                        now.insert(path, key);
                        let same = self.last.get(&path) == Some(&key);
                        counts.same += same as u64;
                        same
                    }
                    None => true,
                };
                let kept = same && kids_kept;
                if kept {
                    counts.whole += tree.performed_input_key(node).is_some() as u64;
                } else {
                    counts.skips += kept_kids;
                }
                match stack.last_mut() {
                    Some(parent) => {
                        parent.3 &= kept;
                        parent.4 += kept as u64;
                    }
                    None => counts.skips += kept as u64,
                }
            }
        }
        self.roots.clear();
        self.last = now;
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::*;

    /// A column root of a 40 px wide box holding a 40 px leaf, and a leaf of
    /// half the root's width.
    fn tree() -> (TaffyTree<()>, NodeId, NodeId) {
        let mut tree = TaffyTree::new();
        let fixed = tree
            .new_leaf(Style {
                size: Size {
                    width: length(40.0),
                    height: length(10.0),
                },
                ..Default::default()
            })
            .unwrap();
        let fluid = tree
            .new_leaf(Style {
                size: Size {
                    width: percent(0.5),
                    height: length(10.0),
                },
                ..Default::default()
            })
            .unwrap();
        let fixed_box = tree
            .new_with_children(
                Style {
                    size: Size {
                        width: length(40.0),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[fixed],
            )
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                &[fixed_box, fluid],
            )
            .unwrap();
        (tree, root, fluid)
    }

    fn layout(tree: &mut TaffyTree<()>, root: NodeId, width: f32) {
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::MaxContent,
            },
        )
        .unwrap();
    }

    #[test]
    fn a_width_change_keeps_the_inputs_of_fixed_subtrees_only() {
        let (mut tree, root, _) = tree();
        let mut noted = Noted::default();
        layout(&mut tree, root, 200.0);
        noted.roots.push(root);
        let first = noted.pass(&tree);
        assert_eq!((first.passes, first.boxes, first.same), (1, 4, 0));
        assert_eq!((first.whole, first.skips), (0, 0));

        noted.roots.push(root);
        let again = noted.pass(&tree);
        assert_eq!((again.boxes, again.same, again.whole, again.skips), (4, 4, 4, 1));

        // Only the root sees the new available width: its auto width shrinks
        // to its 40 px content at both widths, so its items keep their sizes
        // and percentage base. Its two item subtrees are kept whole.
        layout(&mut tree, root, 300.0);
        noted.roots.push(root);
        let wider = noted.pass(&tree);
        assert_eq!((wider.boxes, wider.same, wider.whole, wider.skips), (4, 3, 3, 2));

        // A root of the window's width hands the new width to its items as
        // their percentage base: fewer boxes keep their inputs.
        let mut style = tree.style(root).unwrap().clone();
        style.size.width = percent(1.0);
        tree.set_style(root, style).unwrap();
        layout(&mut tree, root, 200.0);
        noted.roots.push(root);
        noted.pass(&tree);
        layout(&mut tree, root, 300.0);
        noted.roots.push(root);
        let full = noted.pass(&tree);
        assert_eq!(full.boxes, 4);
        assert!(full.same < wider.same, "{full:?}");
    }

    #[test]
    fn a_box_is_named_by_its_path_not_by_its_node() {
        let (mut one, root_one, _) = tree();
        let (mut two, root_two, _) = tree();
        layout(&mut one, root_one, 200.0);
        layout(&mut two, root_two, 200.0);
        let mut noted = Noted::default();
        noted.roots.push(root_one);
        noted.pass(&one);
        noted.roots.push(root_two);
        let other_nodes = noted.pass(&two);
        assert_eq!((other_nodes.boxes, other_nodes.same), (4, 4));
    }
}
