//! Relative insets move painted boxes without moving their exported baseline sets.
use super::taffy_tree::TaffyView;
use crate::prelude::*;
use crate::{Baselines, Direction, LayoutInput, RunMode};

#[test]
fn flex_x_baselines_use_normal_flow_origin_for_both_directions_and_axes() {
    for flow in [
        FlexDirection::Row,
        FlexDirection::RowReverse,
        FlexDirection::Column,
        FlexDirection::ColumnReverse,
    ] {
        for direction in [Direction::Ltr, Direction::Rtl] {
            let mut tree: TaffyTree = TaffyTree::new();
            tree.disable_rounding();
            let child = tree
                .new_leaf(Style {
                    baseline_x_flags: 4,
                    size: Size {
                        width: length(20.0),
                        height: length(20.0),
                    },
                    inset: Rect {
                        left: length(7.0),
                        ..Rect::auto()
                    },
                    ..Style::DEFAULT
                })
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        display: Display::Flex,
                        flex_direction: flow,
                        direction,
                        size: Size {
                            width: length(80.0),
                            height: length(80.0),
                        },
                        ..Style::DEFAULT
                    },
                    &[child],
                )
                .unwrap();
            let mut view = TaffyView {
                taffy: &mut tree,
                measure_function: |inputs, _, _, style: &Style| {
                    let mut output = crate::compute_leaf_layout(
                        inputs,
                        style,
                        |_, _| 0.0,
                        |_, _| Size {
                            width: 20.0,
                            height: 20.0,
                        },
                    );
                    output.baselines_x = Baselines {
                        first: Some(6.0),
                        last: Some(14.0),
                    };
                    output
                },
            };
            let output = crate::compute_flexbox_layout(
                &mut view,
                root,
                LayoutInput {
                    run_mode: RunMode::PerformLayout,
                    ..LayoutInput::HIDDEN
                },
            );
            let normal_x = view.taffy.unrounded_layout(child).location.x - 7.0;
            assert_eq!(
                output.baselines_x.first,
                Some(normal_x + 6.0),
                "{flow:?} {direction:?}"
            );
            assert_eq!(
                output.baselines_x.last,
                Some(normal_x + 14.0),
                "{flow:?} {direction:?}"
            );
        }
    }
}
