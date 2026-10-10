//! Vertical native BFCs retain float packing and auto block-size contribution.
use crate::prelude::*;
use crate::{BlockFlow, Float, Overflow, Point};

#[test]
fn vertical_block_float_packing_uses_inline_height() {
    for reverse in [false, true] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let first = tree
            .new_leaf(Style {
                size: Size::from_lengths(30.0, 45.0),
                float: Float::Left,
                ..Style::DEFAULT
            })
            .unwrap();
        let second = tree
            .new_leaf(Style {
                size: Size::from_lengths(50.0, 45.0),
                float: Float::Left,
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    block_flow: Some(BlockFlow {
                        vertical: true,
                        block_reverse: reverse,
                        inline_reverse: false,
                    }),
                    size: Size {
                        width: auto(),
                        height: length(80.0),
                    },
                    overflow: Point {
                        x: Overflow::Hidden,
                        y: Overflow::Hidden,
                    },
                    ..Style::DEFAULT
                },
                &[first, second],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.layout(root).unwrap().size,
            Size {
                width: 80.0,
                height: 80.0
            }
        );
        assert_eq!(
            tree.layout(first).unwrap().location.x,
            if reverse { 50.0 } else { 0.0 }
        );
        assert_eq!(
            tree.layout(second).unwrap().location.x,
            if reverse { 0.0 } else { 30.0 }
        );
        assert_eq!(tree.layout(first).unwrap().location.y, 0.0);
        assert_eq!(tree.layout(second).unwrap().location.y, 0.0);
    }
}
