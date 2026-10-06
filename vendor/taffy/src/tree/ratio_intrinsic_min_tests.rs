//! Unresolved intrinsic preferred sizes retain aspect-ratio constraint transfers.
use crate::prelude::*;

#[test]
fn intrinsic_leaf_keeps_transferred_minimum_under_flex_parent() {
    for height in [Dimension::AUTO, Dimension::percent(1.0)] {
        for (width, expected) in [
            (Dimension::fit_content(), 100.0),
            (Dimension::fit_content_px(50.0), 100.0),
            (Dimension::min_content(), 100.0),
            (Dimension::max_content(), 100.0),
            (Dimension::length(50.0), 50.0),
            (Dimension::length(0.0), 0.0),
        ] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
            let child = tree
                .new_leaf(Style {
                    display: Display::Block,
                    aspect_ratio: Some(1.0),
                    size: Size { width, height },
                    min_size: Size {
                        width: auto(),
                        height: length(100.0),
                    },
                    ..Style::DEFAULT
                })
                .unwrap();
            let parent = tree
                .new_with_children(
                    Style {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        align_items: Some(AlignItems::FLEX_START),
                        size: Size {
                            width: length(784.0),
                            height: Dimension::AUTO,
                        },
                        ..Style::DEFAULT
                    },
                    &[child],
                )
                .unwrap();
            tree.compute_layout(parent, Size::MAX_CONTENT).unwrap();
            assert_eq!(tree.layout(child).unwrap().size.width, expected);
            assert_eq!(tree.layout(child).unwrap().size.height, 100.0);
        }
    }
}

use crate::{GridLanes, compute_leaf_layout};

fn verify_equal_columns(lanes: bool, bound: u8) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let mut children = Vec::new();
    let mut images = Vec::new();
    for _ in 0..6 {
        let image = tree
            .new_leaf_with_context(
                Style {
                    display: Display::Block,
                    size: Size {
                        width: percent(1.0),
                        height: auto(),
                    },
                    aspect_ratio: Some(113.0 / 120.0),
                    ..Style::DEFAULT
                },
                (),
            )
            .unwrap();
        let pad = 0.0;
        let holder = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    flex_shrink: 0.0,
                    block_flow: Some(crate::BlockFlow {
                        vertical: false,
                        block_reverse: false,
                        inline_reverse: false,
                    }),
                    size: Size {
                        width: percent(1.0),
                        height: auto(),
                    },
                    min_size: Size {
                        width: if bound == 1 { length(100.0) } else { auto() },
                        height: auto(),
                    },
                    max_size: Size {
                        width: if bound == 2 { length(100.0) } else { auto() },
                        height: auto(),
                    },
                    aspect_ratio: Some(113.0 / 120.0),
                    box_sizing: BoxSizing::ContentBox,
                    padding: Rect {
                        left: length(pad),
                        right: length(pad),
                        top: length(pad),
                        bottom: length(pad),
                    },
                    ..Style::DEFAULT
                },
                &[image],
            )
            .unwrap();
        children.push(holder);
        images.push(image);
    }
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                flex_shrink: 0.0,
                block_flow: Some(crate::BlockFlow {
                    vertical: false,
                    block_reverse: false,
                    inline_reverse: false,
                }),
                size: Size {
                    width: length(400.0),
                    height: auto(),
                },
                gap: Size {
                    width: length(10.0),
                    height: length(10.0),
                },
                grid_auto_flow: if lanes {
                    GridAutoFlow::Row
                } else {
                    GridAutoFlow::Row
                },
                grid_template_columns: if lanes {
                    vec![repeat("auto-fill", vec![auto()])]
                } else {
                    vec![auto(); 3]
                },
                grid_lanes: lanes.then_some(GridLanes {
                    rows: false,
                    track_reverse: false,
                    fill_reverse: false,
                    dense: false,
                    tolerance: 0.0,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                ..Style::DEFAULT
            },
            &children,
        )
        .unwrap();
    tree.compute_layout_with_measure(
        root,
        Size {
            width: AvailableSpace::Definite(784.0),
            height: AvailableSpace::Definite(584.0),
        },
        |input, _, _, style| {
            compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, _| Size {
                    width: known.width.unwrap_or(113.0),
                    height: known.height.unwrap_or(120.0),
                },
            )
        },
    )
    .unwrap();
    let expected_width = if bound == 2 { 100.0 } else { 380.0 / 3.0 };
    let expected_height = expected_width * 120.0 / 113.0;
    let mut occupied = [false; 6];
    for (i, &node) in children.iter().enumerate() {
        let layout = tree.layout(node).unwrap();
        assert!(
            (layout.size.width - expected_width).abs() < 0.0001,
            "item {i}: {:?}",
            layout
        );
        assert!(
            (layout.size.height - expected_height).abs() < 0.0001,
            "item {i}: {:?}",
            layout
        );
        // Lanes choose the smallest stack; subpixel height differences can reorder
        // equal-looking items. Every expected geometric slot must still occur once.
        let slot = (0..6)
            .find(|&slot| {
                (layout.location.x - (slot % 3) as f32 * (380.0 / 3.0 + 10.0)).abs() < 0.0001
                    && (layout.location.y - (slot / 3) as f32 * (expected_height + 10.0)).abs()
                        < 0.0001
            })
            .expect("image must occupy an expected geometric slot");
        assert!(!occupied[slot], "duplicate geometric slot {slot}");
        occupied[slot] = true;
        if !lanes {
            assert_eq!(slot, i);
        }
    }
}

#[test]
fn repeated_intrinsic_grid_pass_refreshes_every_equal_image_contribution() {
    for lanes in [false, true] {
        for bound in [0, 1, 2] {
            verify_equal_columns(lanes, bound);
        }
    }
}
