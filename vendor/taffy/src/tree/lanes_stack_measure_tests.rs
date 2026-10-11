//! Row lane placement must use text width after its final inline constraint wraps it.
use crate::prelude::*;
use crate::{compute_leaf_layout, GridLanes, GridPlacement};

fn placement(height: f32, authored_width: Option<f32>) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let row = Line {
        start: GridPlacement::Line(1.into()),
        end: GridPlacement::Auto,
    };
    let text = tree
        .new_leaf_with_context(
            Style {
                grid_row: row.clone(),
                size: Size {
                    width: authored_width.map_or(Dimension::auto(), Dimension::length),
                    height: auto(),
                },
                ..Style::DEFAULT
            },
            (),
        )
        .unwrap();
    let next = tree
        .new_leaf(Style {
            grid_row: row,
            size: Size {
                width: length(10.0),
                height: length(10.0),
            },
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: length(200.0),
                    height: length(height),
                },
                grid_template_rows: vec![length(height)],
                grid_lanes: Some(GridLanes {
                    rows: true,
                    track_reverse: false,
                    fill_reverse: false,
                    dense: false,
                    tolerance: 0.0,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                gap: Size {
                    width: length(7.0),
                    height: length(0.0),
                },
                ..Style::DEFAULT
            },
            &[text, next],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
        compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |known, available| {
                let height = known.height.unwrap_or(match available.height {
                    AvailableSpace::Definite(value) => value.min(75.0),
                    AvailableSpace::MinContent => 40.0,
                    AvailableSpace::MaxContent => 75.0,
                });
                // A synthetic vertical paragraph occupies two columns below 75px.
                let width = if height < 75.0 { 40.0 } else { 20.0 };
                Size {
                    width: known.width.unwrap_or(width),
                    height,
                }
            },
        )
    })
    .unwrap();
    (
        tree.layout(text).unwrap().size.width,
        tree.layout(next).unwrap().location.x,
    )
}

#[test]
fn wrapped_row_lane_advances_past_all_text_columns() {
    assert_eq!(placement(30.0, None), (40.0, 47.0));
}

#[test]
fn unwrapped_row_lane_keeps_one_column() {
    assert_eq!(placement(100.0, None), (20.0, 27.0));
}

#[test]
fn authored_row_lane_width_still_controls_placement() {
    for height in [30.0, 100.0] {
        assert_eq!(placement(height, Some(55.0)), (55.0, 62.0));
    }
}
