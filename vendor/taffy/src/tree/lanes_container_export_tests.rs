//! Parent flex placement verifies that native lane container baselines reach consumers.
use crate::style_helpers::*;
use crate::{AlignItems, Display, GridLanes, Line, MeasureOutput, Size, Style, TaffyTree};

fn parent_positions(alignment: AlignItems) -> (f32, f32, f32) {
    let mut tree: TaffyTree<(f32, f32, f32)> = TaffyTree::new();
    tree.disable_rounding();
    let mut children = vec![];
    for (track, height, first, last) in [
        (1, 40.0, 30.0, 35.0),
        (2, 60.0, 15.0, 20.0),
        (1, 20.0, 5.0, 10.0),
        (2, 20.0, 5.0, 10.0),
    ] {
        children.push(
            tree.new_leaf_with_context(
                Style {
                    size: Size {
                        width: length(50.0),
                        height: length(height),
                    },
                    grid_column: Line {
                        start: line(track),
                        end: span(1),
                    },
                    ..Style::DEFAULT
                },
                (height, first, last),
            )
            .unwrap(),
        );
    }
    let lanes = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: length(100.0),
                    height: auto(),
                },
                grid_template_columns: vec![length(50.0), length(50.0)],
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
    let reference = tree
        .new_leaf_with_context(
            Style {
                size: Size {
                    width: length(20.0),
                    height: length(20.0),
                },
                ..Style::DEFAULT
            },
            (20.0, 10.0, 10.0),
        )
        .unwrap();
    let parent = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                align_items: alignment,
                ..Style::DEFAULT
            },
            &[lanes, reference],
        )
        .unwrap();
    tree.compute_layout_with_measure(parent, Size::MAX_CONTENT, |input, _, context, style| {
        let (height, first, last) = context.copied().unwrap_or((0.0, 0.0, 0.0));
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |_, _| MeasureOutput {
                size: Size {
                    width: 50.0,
                    height,
                },
                baseline: Some(first),
                last_baseline: Some(last),
                baseline_x: None,
                last_baseline_x: None,
                baseline_x_from_right: false,
            },
        )
    })
    .unwrap();
    (
        tree.layout(lanes).unwrap().location.y,
        tree.layout(reference).unwrap().location.y,
        tree.layout(lanes).unwrap().size.height,
    )
}

#[test]
fn parent_first_baseline_uses_highest_usable_track_baseline() {
    assert_eq!(parent_positions(AlignItems::BASELINE), (0.0, 5.0, 80.0));
}

#[test]
fn parent_last_baseline_uses_lowest_last_track_baseline() {
    assert_eq!(
        parent_positions(AlignItems::LAST_BASELINE),
        (0.0, 60.0, 80.0)
    );
}
