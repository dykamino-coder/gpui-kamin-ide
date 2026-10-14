//! A fractional standalone axis wraps text like equivalent fixed tracks in row lanes.
use crate::prelude::*;
use crate::{compute_leaf_layout, GridLanes, SUBGRID_ROWS};

fn height(fractional: bool) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let text = tree.new_leaf_with_context(Style::DEFAULT, ()).unwrap();
    let inner = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                subgrid: SUBGRID_ROWS,
                grid_template_columns: if fractional {
                    vec![fr(1.0); 4]
                } else {
                    vec![length(25.0); 4]
                },
                ..Style::DEFAULT
            },
            &[text],
        )
        .unwrap();
    let outer = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                subgrid: SUBGRID_ROWS,
                ..Style::DEFAULT
            },
            &[inner],
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: length(100.0),
                    height: Dimension::AUTO,
                },
                grid_template_rows: vec![auto()],
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
            &[outer],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
        compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |known, available| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(width) => width.clamp(25.0, 175.0),
                    AvailableSpace::MinContent => 25.0,
                    AvailableSpace::MaxContent => 175.0,
                });
                let words = ((width + 25.0) / 50.0).floor().max(1.0);
                Size {
                    width,
                    height: known.height.unwrap_or((4.0 / words).ceil() * 25.0),
                }
            },
        )
    })
    .unwrap();
    tree.layout(root).unwrap().size.height
}

#[test]
fn fractional_standalone_columns_keep_row_lane_text_wrapping() {
    assert_eq!(height(false), 100.0);
    assert_eq!(height(true), 100.0);
}
