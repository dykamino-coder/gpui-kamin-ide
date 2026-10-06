//! Compare intrinsic block-axis flexible tracks with the independent browser geometry.
use crate::prelude::*;
use crate::{BlockFlow, GridLanes};

fn height(lanes: bool, minimum: bool, nested: bool) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let children: Vec<_> = (0..5)
        .map(|index| {
            tree.new_leaf(Style {
                size: Size::from_lengths(10.0, if index == 0 { 20.0 } else { 10.0 }),
                ..Style::DEFAULT
            })
            .unwrap()
        })
        .collect();
    let grid = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: Dimension::max_content(),
                    height: if minimum {
                        Dimension::min_content()
                    } else {
                        Dimension::max_content()
                    },
                },
                grid_template_rows: vec![fr(1.0), fr(2.0), fr(1.0), fr(1.0)],
                grid_auto_flow: GridAutoFlow::Column,
                block_flow: Some(BlockFlow {
                    vertical: false,
                    block_reverse: false,
                    inline_reverse: false,
                }),
                grid_lanes: lanes.then_some(GridLanes {
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
    let root = if nested {
        tree.new_with_children(
            Style {
                display: Display::Block,
                size: Size {
                    width: length(300.0),
                    height: auto(),
                },
                ..Style::DEFAULT
            },
            &[grid],
        )
        .unwrap()
    } else {
        grid
    };
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    tree.layout(grid).unwrap().size.height
}

#[test]
fn authored_block_min_content_matches_automatic_fractional_track_geometry() {
    let cases: Vec<_> = [false, true]
        .into_iter()
        .flat_map(|lanes| {
            [false, true].into_iter().flat_map(move |minimum| {
                [false, true]
                    .into_iter()
                    .map(move |nested| (lanes, minimum, nested, height(lanes, minimum, nested)))
            })
        })
        .collect();
    for (lanes, minimum, nested, height) in &cases {
        println!("lanes={lanes} minimum={minimum} nested={nested} height={height}");
    }
    for (lanes, minimum, nested, height) in cases {
        assert_eq!(
            height, 100.0,
            "lanes={lanes} minimum={minimum} nested={nested}"
        );
    }
}
