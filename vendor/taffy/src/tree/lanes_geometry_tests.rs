//! Grid-lanes geometry must agree with real track edges and safe overflow.
use crate::style_helpers::*;
use crate::{
    AlignContent, AlignItems, AvailableSpace, Display, GridLanes, GridPlacement, LengthPercentage,
    Line, Position, Rect, Size, Style, TaffyTree,
};

fn lanes(rows: bool, reverse: bool) -> GridLanes {
    GridLanes {
        rows,
        fill_reverse: reverse,
        track_reverse: false,
        dense: false,
        tolerance: 0.0,
        tolerance_pct: None,
        stack_block: false,
    }
}

fn absolute(rows: bool, start: i16, end: i16, align: AlignItems) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let placement = Line {
        start: GridPlacement::Line(start.into()),
        end: GridPlacement::Line(end.into()),
    };
    let child = tree
        .new_leaf(Style {
            position: Position::Absolute,
            size: Size::from_lengths(20.0, 20.0),
            grid_column: if rows {
                Line::default()
            } else {
                placement.clone()
            },
            grid_row: if rows { placement } else { Line::default() },
            justify_self: Some(align),
            align_self: Some(align),
            ..Style::DEFAULT
        })
        .unwrap();
    let tracks = vec![length(100.0), length(150.0)];
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_lanes: Some(lanes(rows, false)),
                size: Size::from_lengths(300.0, 300.0),
                grid_template_columns: if rows { vec![] } else { tracks.clone() },
                grid_template_rows: if rows { tracks } else { vec![] },
                gap: Size {
                    width: LengthPercentage::length(10.0),
                    height: LengthPercentage::length(10.0),
                },
                padding: Rect {
                    left: LengthPercentage::length(10.0),
                    right: LengthPercentage::length(10.0),
                    top: LengthPercentage::length(10.0),
                    bottom: LengthPercentage::length(10.0),
                },
                ..Style::DEFAULT
            },
            &[child],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let l = tree.layout(child).unwrap();
    if rows {
        (l.location.y, l.size.height)
    } else {
        (l.location.x, l.size.width)
    }
}

#[test]
fn abspos_track_start_excludes_gap_in_both_axes() {
    for rows in [false, true] {
        assert_eq!(absolute(rows, 2, 3, AlignItems::START), (120.0, 20.0));
    }
}
#[test]
fn abspos_track_end_excludes_gap_in_both_axes() {
    for rows in [false, true] {
        assert_eq!(absolute(rows, 1, 2, AlignItems::END), (90.0, 20.0));
    }
}
#[test]
fn abspos_center_uses_real_track_width_in_both_axes() {
    for rows in [false, true] {
        assert_eq!(absolute(rows, 2, 3, AlignItems::CENTER), (185.0, 20.0));
    }
}

fn reverse_positions(rows: bool, align: AlignContent) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let first = tree
        .new_leaf(Style {
            size: Size::from_lengths(40.0, 40.0),
            ..Style::DEFAULT
        })
        .unwrap();
    let second = tree
        .new_leaf(Style {
            size: Size::from_lengths(40.0, 40.0),
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_lanes: Some(lanes(rows, true)),
                size: if rows {
                    Size::from_lengths(60.0, 100.0)
                } else {
                    Size::from_lengths(100.0, 60.0)
                },
                grid_template_columns: if rows { vec![] } else { vec![length(80.0)] },
                grid_template_rows: if rows { vec![length(80.0)] } else { vec![] },
                justify_content: Some(align),
                align_content: Some(align),
                gap: Size {
                    width: LengthPercentage::length(4.0),
                    height: LengthPercentage::length(4.0),
                },
                ..Style::DEFAULT
            },
            &[first, second],
        )
        .unwrap();
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
    )
    .unwrap();
    let a = tree.layout(first).unwrap().location;
    let b = tree.layout(second).unwrap().location;
    if rows {
        (a.x, b.x)
    } else {
        (a.y, b.y)
    }
}
#[test]
fn safe_reverse_overflow_retains_end_anchor_in_both_axes() {
    for rows in [false, true] {
        for align in [
            AlignContent::SAFE_START,
            AlignContent::SAFE_CENTER,
            AlignContent::SAFE_END,
        ] {
            assert_eq!(reverse_positions(rows, align), (20.0, -24.0));
        }
    }
}

#[test]
fn absolute_first_and_last_grid_lines_keep_real_edges() {
    for rows in [false, true] {
        assert_eq!(absolute(rows, 1, 1, AlignItems::START), (10.0, 20.0));
        assert_eq!(absolute(rows, 3, 3, AlignItems::START), (270.0, 20.0));
        assert_eq!(absolute(rows, 1, 3, AlignItems::CENTER), (130.0, 20.0));
    }
}
#[test]
fn unsafe_reverse_center_preserves_overflow_distribution() {
    for rows in [false, true] {
        assert_eq!(reverse_positions(rows, AlignContent::CENTER), (32.0, -12.0));
    }
}
