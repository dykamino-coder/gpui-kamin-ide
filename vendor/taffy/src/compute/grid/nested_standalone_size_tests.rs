//! Compare nested fixed and fractional standalone rows under identical definite parent tracks.
use crate::prelude::*;
use crate::{compute_leaf_layout, GridLanes};

fn width(lanes: bool, fractional: bool) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let text = tree.new_leaf_with_context(Style::DEFAULT, ()).unwrap();
    let inner = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                subgrid: crate::style::SUBGRID_COLUMNS,
                grid_template_rows: if fractional {
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
                subgrid: crate::style::SUBGRID_COLUMNS,
                grid_template_rows: vec![length(100.0)],
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
                    width: Dimension::min_content(),
                    height: length(100.0),
                },
                grid_template_columns: vec![auto()],
                grid_lanes: lanes.then_some(GridLanes {
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
            &[outer],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
        compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |known, available| {
                // Ahem X X X X: four 25px words with three 25px collapsible spaces.
                // A 25px standalone row fits one word; unconstrained height fits all seven cells.
                let height = known.height.unwrap_or(match available.height {
                    AvailableSpace::Definite(height) => height.clamp(25.0, 175.0),
                    AvailableSpace::MinContent => 25.0,
                    AvailableSpace::MaxContent => 175.0,
                });
                let cells_per_column = ((height + 25.0) / 50.0).floor().max(1.0);
                Size {
                    width: known
                        .width
                        .unwrap_or((4.0 / cells_per_column).ceil() * 25.0),
                    height,
                }
            },
        )
    })
    .unwrap();
    tree.layout(root).unwrap().size.width
}

#[test]
fn nested_fractional_tracks_use_definite_standalone_grid_item_size() {
    let cases: Vec<_> = [false, true]
        .into_iter()
        .flat_map(|lanes| {
            [false, true]
                .into_iter()
                .map(move |fractional| (lanes, fractional, width(lanes, fractional)))
        })
        .collect();
    for (lanes, fractional, width) in &cases {
        println!("lanes={lanes} fractional={fractional} root_width={width}");
    }
    for (lanes, fractional, width) in cases {
        assert_eq!(width, 100.0, "lanes={lanes} fractional={fractional}");
    }
}
