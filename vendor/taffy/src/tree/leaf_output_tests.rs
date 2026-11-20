//! KaminIDE patch: a leaf repeating a laid out subtree's output (`LeafOutput`)
//! is placed and collapses its margins exactly as the subtree.

use crate::style_helpers::*;
use crate::{Display, LeafOutput, NodeId, Rect, Size, Style, TaffyTree};

fn block(height: f32, top: f32, bottom: f32) -> Style {
    Style {
        display: Display::Block,
        size: Size { width: auto(), height: length(height) },
        margin: Rect { left: zero(), right: zero(), top: length(top), bottom: length(bottom) },
        ..Style::DEFAULT
    }
}

/// A block whose first and last children's margins collapse through its
/// edges, between two siblings: `(tree, root, [before, middle, after])`.
fn page(inner: (f32, f32), middle: Option<Style>) -> (TaffyTree<()>, NodeId, [NodeId; 3]) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let before = tree.new_leaf(block(10.0, 0.0, 4.0)).unwrap();
    let middle = match middle {
        Some(leaf) => tree.new_leaf(leaf).unwrap(),
        None => {
            let first = tree.new_leaf(block(20.0, inner.0, 0.0)).unwrap();
            let last = tree.new_leaf(block(20.0, 0.0, inner.1)).unwrap();
            let style = Style { size: Size { width: auto(), height: auto() }, ..block(0.0, 3.0, 2.0) };
            tree.new_with_children(style, &[first, last]).unwrap()
        }
    };
    let after = tree.new_leaf(block(10.0, 5.0, 0.0)).unwrap();
    let root_style = Style {
        display: Display::Block,
        size: Size { width: length(100.0), height: auto() },
        ..Style::DEFAULT
    };
    let root = tree.new_with_children(root_style, &[before, middle, after]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    (tree, root, [before, middle, after])
}

fn places(inner: (f32, f32)) {
    let (tree, root, [_, middle, after]) = page(inner, None);
    let built = tree.layout(middle).unwrap();
    let output = tree.computed_layout_output(middle).unwrap().expect("laid out");
    let leaf = Style {
        size: Size { width: auto(), height: length(built.size.height) },
        leaf_output: Some(Box::new(LeafOutput::of(&output))),
        ..block(0.0, 3.0, 2.0)
    };
    let (stand, stand_root, [_, leaf_node, stand_after]) = page(inner, Some(leaf));
    assert_eq!(stand.layout(leaf_node).unwrap().location, built.location, "leaf at the block's place");
    assert_eq!(stand.layout(stand_after).unwrap().location, tree.layout(after).unwrap().location);
    assert_eq!(stand.layout(stand_root).unwrap().size, tree.layout(root).unwrap().size);
}

#[test]
fn a_leaf_collapses_the_margins_of_the_subtree_edges_as_the_subtree() {
    places((16.0, 24.0));
}

#[test]
fn margins_of_both_signs_collapse_exactly_through_the_leaf() {
    places((-7.0, -9.0));
    places((12.0, -30.0));
}
