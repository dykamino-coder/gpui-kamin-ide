//! Baseline fallback follows the projected item's block-start and block-end edges.
use crate::style_helpers::*;
use crate::{AlignItems, Direction, Display, GridLanes, Size, Style, TaffyTree};
fn offset(lanes: bool, flags: u8, last: bool, direction: Direction) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let leaf = tree
        .new_leaf(Style {
            size: Size::from_lengths(20.0, 20.0),
            baseline_x_flags: flags,
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                direction,
                size: Size::from_lengths(80.0, 80.0),
                grid_template_columns: vec![length(80.0)],
                grid_template_rows: if lanes { vec![] } else { vec![length(80.0)] },
                justify_items: Some(if last {
                    AlignItems::LAST_BASELINE
                } else {
                    AlignItems::BASELINE
                }),
                grid_lanes: lanes.then_some(GridLanes {
                    rows: false,
                    track_reverse: false,
                    fill_reverse: true,
                    dense: false,
                    tolerance: 0.0,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                ..Style::DEFAULT
            },
            &[leaf],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    tree.layout(leaf).unwrap().location.x
}
#[test]
fn vertical_rl_last_fallback_uses_the_physical_left_edge() {
    for lanes in [false, true] {
        assert_eq!(offset(lanes, 5, true, Direction::Ltr), 0.0);
    }
}
#[test]
fn vertical_lr_last_fallback_uses_the_physical_right_edge() {
    for lanes in [false, true] {
        assert_eq!(offset(lanes, 4, true, Direction::Ltr), 60.0);
    }
}
#[test]
fn first_fallback_preserves_the_opposite_orientation() {
    for lanes in [false, true] {
        assert_eq!(offset(lanes, 5, false, Direction::Ltr), 60.0);
        assert_eq!(offset(lanes, 4, false, Direction::Ltr), 0.0);
    }
}
#[test]
fn native_rtl_does_not_flip_physical_baseline_orientation_twice() {
    assert_eq!(offset(false, 5, true, Direction::Rtl), 0.0);
    assert_eq!(offset(false, 4, true, Direction::Rtl), 60.0);
}
