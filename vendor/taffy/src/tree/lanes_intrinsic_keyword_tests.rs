//! Authored intrinsic grid height survives a parent min-content contribution probe.
use crate::compute_leaf_layout;
use crate::prelude::*;
#[test]
fn authored_max_content_lane_height_survives_min_content_probe() {
    for lanes in [false, true] {
        assert_eq!(
            contribution(lanes),
            Size {
                width: 42.0,
                height: 153.0
            }
        );
    }
}

fn contribution(lanes: bool) -> Size<f32> {
    let mut tree: TaffyTree<f32> = TaffyTree::new();
    tree.disable_rounding();
    let mut children = Vec::new();
    for (index, natural_height) in [10.0, 30.0, 30.0, 10.0, 30.0].into_iter().enumerate() {
        children.push(
            tree.new_leaf_with_context(
                Style {
                    size: Size {
                        width: auto(),
                        height: if index == 0 { length(20.0) } else { auto() },
                    },
                    ..Style::DEFAULT
                },
                natural_height,
            )
            .unwrap(),
        );
    }
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: auto(),
                    height: Dimension::max_content(),
                },
                grid_template_rows: vec![fr(1.0), fr(2.0), fr(1.0), fr(1.0)],
                grid_template_columns: vec![auto()],
                grid_auto_flow: GridAutoFlow::Column,
                grid_lanes: lanes.then_some(crate::GridLanes {
                    rows: true,
                    track_reverse: false,
                    fill_reverse: false,
                    dense: false,
                    tolerance: 0.0,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                gap: Size {
                    width: length(2.0),
                    height: length(1.0),
                },
                ..Style::DEFAULT
            },
            &children,
        )
        .unwrap();
    tree.compute_layout_with_measure(
        root,
        Size {
            width: AvailableSpace::MinContent,
            height: AvailableSpace::MinContent,
        },
        |input, _, context, style| {
            let natural = *context.unwrap();
            compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, available| {
                    let height = known.height.unwrap_or(match available.height {
                        AvailableSpace::Definite(value) => value.min(natural),
                        AvailableSpace::MinContent => 10.0,
                        AvailableSpace::MaxContent => natural,
                    });
                    Size {
                        width: known
                            .width
                            .unwrap_or(if height < natural { 40.0 } else { 20.0 }),
                        height,
                    }
                },
            )
        },
    )
    .unwrap();
    tree.layout(root).unwrap().size
}
