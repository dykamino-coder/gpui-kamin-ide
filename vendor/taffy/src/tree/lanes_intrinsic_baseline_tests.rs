//! Native baseline shims must contribute before intrinsic lane tracks are sized.
use crate::style_helpers::*;
use crate::tree::MeasureOutput;
use crate::{AlignItems, Display, GridLanes, LengthPercentage, Rect, Size, Style, TaffyTree};

fn rows(
    spec: &[(f32, f32, f32, i16, u16)],
    tracks: Vec<crate::GridTemplateComponent<String>>,
) -> (f32, Vec<f32>) {
    let mut tree: TaffyTree<f32> = TaffyTree::new();
    tree.disable_rounding();
    let mut children = vec![];
    for &(height, top, bottom, start, count) in spec {
        children.push(
            tree.new_leaf_with_context(
                Style {
                    size: Size {
                        width: length(50.0),
                        height: auto(),
                    },
                    padding: Rect {
                        top: LengthPercentage::length(top),
                        bottom: LengthPercentage::length(bottom),
                        left: zero(),
                        right: zero(),
                    },
                    grid_row: crate::Line {
                        start: line(start),
                        end: span(count),
                    },
                    ..Style::DEFAULT
                },
                height,
            )
            .unwrap(),
        );
    }
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: length(200.0),
                    height: auto(),
                },
                align_items: Some(AlignItems::BASELINE),
                grid_template_rows: tracks,
                grid_lanes: Some(GridLanes {
                    rows: true,
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
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, context, style| {
        let height = *context.unwrap();
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |_, _| MeasureOutput {
                size: Size {
                    width: 50.0,
                    height,
                },
                baseline: Some(height * 0.8),
                last_baseline: Some(height * 0.8),
                baseline_x: None,
                last_baseline_x: None,
                baseline_x_from_right: false,
            },
        )
    })
    .unwrap();
    (
        tree.layout(root).unwrap().size.height,
        children
            .iter()
            .map(|&n| tree.layout(n).unwrap().location.y)
            .collect(),
    )
}

#[test]
fn one_span_baselines_preserve_the_intrinsic_row_floor() {
    assert_eq!(
        rows(
            &[(20.0, 20.0, 0.0, 1, 1), (20.0, 0.0, 0.0, 1, 1)],
            vec![minmax(length(0.0), auto())]
        ),
        (40.0, vec![0.0, 20.0])
    );
}
#[test]
fn spanning_baseline_grows_the_start_track_before_distribution() {
    assert_eq!(
        rows(
            &[(20.0, 0.0, 40.0, 1, 1), (20.0, 50.0, 0.0, 1, 2)],
            vec![minmax(length(0.0), auto()), minmax(length(0.0), auto())]
        ),
        (110.0, vec![50.0, 0.0])
    );
}
#[test]
fn spanning_baseline_respects_the_fixed_middle_track() {
    assert_eq!(
        rows(
            &[
                (20.0, 0.0, 20.0, 1, 1),
                (10.0, 0.0, 0.0, 3, 1),
                (20.0, 50.0, 0.0, 1, 3)
            ],
            vec![
                minmax(length(0.0), auto()),
                length(30.0),
                minmax(length(0.0), auto())
            ]
        ),
        (130.0, vec![50.0, 120.0, 0.0])
    );
}
