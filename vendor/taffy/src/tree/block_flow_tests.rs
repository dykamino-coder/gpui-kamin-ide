//! Native block flow matches browser coordinates and collapses margins along its actual block axis.
use crate::prelude::*;
use crate::{BlockFlow, BoxSizing};

#[test]
fn vertical_root_fills_inline_height_and_keeps_auto_block_width() {
    for reverse in [false, true] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let child = tree
            .new_leaf(Style {
                size: Size::from_lengths(25.0, 20.0),
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
                    ..Style::DEFAULT
                },
                &[child],
            )
            .unwrap();
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(120.0),
            },
        )
        .unwrap();
        assert_eq!(
            tree.layout(root).unwrap().size,
            Size {
                width: 25.0,
                height: 120.0
            }
        );
        assert_eq!(
            tree.layout(root).unwrap().location.x,
            if reverse { 175.0 } else { 0.0 }
        );
        assert_eq!(tree.layout(child).unwrap().location.x, 0.0);
    }
}

#[test]
fn block_writing_modes_match_chromium_coordinates() {
    // Independently measured with Chromium, with asymmetric padding and borders.
    for (vertical, block_reverse, inline_reverse, x, y) in [
        (false, false, false, 40.0, 10.0),
        (true, true, false, 120.0, 10.0),
        (true, false, false, 40.0, 10.0),
        (false, false, true, 110.0, 10.0),
        (true, true, true, 120.0, 55.0),
        (true, false, true, 40.0, 55.0),
    ] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let child = tree
            .new_leaf(Style {
                size: if vertical {
                    Size::from_lengths(20.0, 30.0)
                } else {
                    Size::from_lengths(30.0, 20.0)
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    box_sizing: BoxSizing::ContentBox,
                    size: Size::from_lengths(100.0, 75.0),
                    padding: Rect {
                        left: length(20.0),
                        right: length(10.0),
                        top: length(5.0),
                        bottom: length(15.0),
                    },
                    border: Rect {
                        left: length(20.0),
                        right: length(10.0),
                        top: length(5.0),
                        bottom: length(15.0),
                    },
                    block_flow: Some(BlockFlow {
                        vertical,
                        block_reverse,
                        inline_reverse,
                    }),
                    ..Style::DEFAULT
                },
                &[child],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let child = tree.layout(child).unwrap();
        assert_eq!(
            child.location.x, x,
            "vertical={vertical} block={block_reverse} inline={inline_reverse}"
        );
        assert_eq!(child.location.y, y);
        assert_eq!(
            child.size,
            if vertical {
                Size {
                    width: 20.0,
                    height: 30.0,
                }
            } else {
                Size {
                    width: 30.0,
                    height: 20.0,
                }
            }
        );
        assert_eq!(
            tree.layout(root).unwrap().size,
            Size {
                width: 160.0,
                height: 115.0
            }
        );
    }
}

#[test]
fn vertical_block_margins_collapse_without_flex_sizing() {
    for reverse in [false, true] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let flow = BlockFlow {
            vertical: true,
            block_reverse: reverse,
            inline_reverse: false,
        };
        let first = tree
            .new_leaf(Style {
                display: Display::Block,
                block_flow: Some(flow),
                size: Size::from_lengths(20.0, 30.0),
                margin: Rect {
                    left: length(if reverse { 10.0 } else { 0.0 }),
                    right: length(if reverse { 0.0 } else { 10.0 }),
                    top: length(0.0),
                    bottom: length(0.0),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let second = tree
            .new_leaf(Style {
                display: Display::Block,
                block_flow: Some(flow),
                size: Size::from_lengths(40.0, 30.0),
                margin: Rect {
                    left: length(if reverse { 0.0 } else { 20.0 }),
                    right: length(if reverse { 20.0 } else { 0.0 }),
                    top: length(0.0),
                    bottom: length(0.0),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    block_flow: Some(flow),
                    size: Size {
                        width: auto(),
                        height: length(100.0),
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
                height: 100.0
            }
        );
        assert_eq!(
            tree.layout(first).unwrap().location.x,
            if reverse { 60.0 } else { 0.0 }
        );
        assert_eq!(
            tree.layout(second).unwrap().location.x,
            if reverse { 0.0 } else { 40.0 }
        );
    }
}
