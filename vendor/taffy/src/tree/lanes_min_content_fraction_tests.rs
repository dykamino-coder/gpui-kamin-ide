//! A min-content lane constraint must survive final layout of unequal flexible tracks.

use crate::style_helpers::*;
use crate::{AvailableSpace, Display, GridLanes, Size, Style, TaffyTree};

fn width(wide: usize, available: AvailableSpace, mode: u8) -> f32 {
    let mut tree: TaffyTree = TaffyTree::new();
    tree.disable_rounding();
    let children: Vec<_> = (0..5)
        .map(|index| {
            tree.new_leaf(Style {
                size: Size {
                    width: length(if index == wide { 20.0 } else { 10.0 }),
                    height: length(10.0),
                },
                ..Style::DEFAULT
            })
            .unwrap()
        })
        .collect();
    let lanes = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: if mode == 1 {
                        crate::Dimension::min_content()
                    } else {
                        auto()
                    },
                    height: auto(),
                },
                grid_template_columns: vec![fr(1.0), fr(2.0), fr(1.0), fr(1.0)],
                grid_lanes: Some(GridLanes {
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
    let child = if mode == 2 {
        tree.new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: crate::Dimension::min_content(),
                    height: auto(),
                },
                grid_template_columns: vec![min_content()],
                justify_items: crate::AlignItems::START,
                ..Style::DEFAULT
            },
            &[lanes],
        )
        .unwrap()
    } else {
        lanes
    };
    let root = if mode != 0 {
        tree.new_with_children(
            Style {
                display: Display::Block,
                size: Size {
                    width: length(300.0),
                    height: auto(),
                },
                ..Style::DEFAULT
            },
            &[child],
        )
        .unwrap()
    } else {
        lanes
    };
    tree.compute_layout(
        root,
        Size {
            width: available,
            height: AvailableSpace::MaxContent,
        },
    )
    .unwrap();
    tree.layout(lanes).unwrap().size.width
}

#[test]
fn a_wide_first_auto_placed_item_sets_each_min_content_track_floor() {
    assert_eq!(width(0, AvailableSpace::MinContent, 0), 80.0);
    assert_eq!(width(0, AvailableSpace::MaxContent, 0), 100.0);
}

#[test]
fn a_wide_later_auto_placed_item_sets_each_min_content_track_floor() {
    assert_eq!(width(4, AvailableSpace::MinContent, 0), 80.0);
    assert_eq!(width(4, AvailableSpace::MaxContent, 0), 100.0);
}

#[test]
fn authored_min_content_keeps_intrinsic_floors_in_final_block_child_layout() {
    assert_eq!(width(0, AvailableSpace::MaxContent, 1), 80.0);
    assert_eq!(width(4, AvailableSpace::MaxContent, 1), 80.0);
}

#[test]
fn min_content_grid_wrapper_preserves_the_same_child_intrinsic_width() {
    assert_eq!(width(0, AvailableSpace::MaxContent, 2), 80.0);
    assert_eq!(width(4, AvailableSpace::MaxContent, 2), 80.0);
}
