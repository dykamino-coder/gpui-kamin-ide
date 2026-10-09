//! Absolute lanes share physical self-alignment with ordinary grid areas.
use crate::prelude::*;
use crate::{AlignItems, Direction, GridLanes, GridPlacement, Point, Position};

fn position(rows: bool, reversed: bool, alignment: AlignItems, lanes: bool) -> Point<f32> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let definite = Line {
        start: GridPlacement::Line(1.into()),
        end: GridPlacement::Line(2.into()),
    };
    let child = tree
        .new_leaf(Style {
            position: Position::Absolute,
            size: Size::from_lengths(30.0, 20.0),
            grid_column: if rows {
                Line::default()
            } else {
                definite.clone()
            },
            grid_row: if rows { definite } else { Line::default() },
            justify_self: Some(alignment),
            align_self: Some(AlignItems::START),
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                // The grid must be the containing block (Static is the default since upstream
                // #1140) for grid placement to apply to the abspos child.
                position: Position::Relative,
                direction: if reversed {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                },
                grid_axis_reversed: Some(Size {
                    width: reversed,
                    height: false,
                }),
                grid_lanes: lanes.then_some(GridLanes {
                    rows,
                    fill_reverse: false,
                    track_reverse: false,
                    dense: false,
                    tolerance: 0.0,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                size: Size::from_lengths(320.0, 220.0),
                padding: Rect {
                    left: length(10.0),
                    right: length(10.0),
                    top: length(10.0),
                    bottom: length(10.0),
                },
                gap: Size {
                    width: length(10.0),
                    height: length(10.0),
                },
                grid_template_columns: vec![length(100.0), length(150.0)],
                grid_template_rows: vec![length(70.0), length(110.0)],
                ..Style::DEFAULT
            },
            &[child],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    tree.layout(child).unwrap().location
}

#[test]
fn absolute_lane_self_alignment_matches_ordinary_grid_in_both_directions() {
    for rows in [false, true] {
        for reversed in [false, true] {
            for alignment in [AlignItems::START, AlignItems::END, AlignItems::CENTER] {
                assert_eq!(
                    position(rows, reversed, alignment, true),
                    position(rows, reversed, alignment, false)
                );
            }
        }
    }
}

#[test]
fn absolute_column_start_respects_physical_right_to_left_track_edges() {
    assert_eq!(position(false, false, AlignItems::START, false).x, 10.0);
    assert_eq!(position(false, true, AlignItems::START, false).x, 280.0);
    assert_eq!(position(false, true, AlignItems::START, true).x, 280.0);
}
