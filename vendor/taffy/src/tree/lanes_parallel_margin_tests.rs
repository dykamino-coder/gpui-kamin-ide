//! Compare column-lane and grid fallback baselines with asymmetric inline margins.
use crate::{prelude::*, GridLanes};
fn positions(lanes: bool, last: bool) -> Vec<(f32, f32)> {
    let mut t: TaffyTree<()> = TaffyTree::new();
    t.disable_rounding();
    let mut items = vec![];
    for (i, (width, left, right)) in [
        (60., 0., 0.),
        (70., 15., 15.),
        (75., 5., 10.),
        (80., 10., 5.),
        (90., 0., 20.),
        (55., 25., 5.),
        (50., 20., 0.),
        (60., 15., 0.),
        (85., 5., 25.),
    ]
    .into_iter()
    .enumerate()
    {
        items.push(
            t.new_leaf(Style {
                display: Display::Block,
                size: Size {
                    width: length(width),
                    height: length(35.),
                },
                margin: Rect {
                    left: length(left),
                    right: length(right),
                    top: length(0.),
                    bottom: length(0.),
                },
                baseline_x_flags: 8,
                grid_column: Line {
                    start: line((i % 3 + 1) as i16),
                    end: auto(),
                },
                grid_row: if lanes {
                    Line::default()
                } else {
                    Line {
                        start: line((i / 3 + 1) as i16),
                        end: auto(),
                    }
                },
                ..Style::DEFAULT
            })
            .unwrap(),
        );
    }
    let root = t
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_lanes: lanes.then_some(GridLanes {
                    rows: false,
                    fill_reverse: false,
                    track_reverse: false,
                    dense: false,
                    tolerance: 0.,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                grid_template_columns: vec![length(120.); 3],
                grid_template_rows: if lanes {
                    vec![]
                } else {
                    vec![min_content(); 3]
                },
                gap: Size {
                    width: length(3.),
                    height: length(3.),
                },
                justify_items: if last {
                    AlignItems::LAST_BASELINE
                } else {
                    AlignItems::BASELINE
                },
                size: Size {
                    width: length(400.),
                    height: Dimension::AUTO,
                },
                ..Style::DEFAULT
            },
            &items,
        )
        .unwrap();
    t.compute_layout(root, Size::MAX_CONTENT).unwrap();
    items
        .iter()
        .map(|&item| {
            let l = t.layout(item).unwrap();
            (l.location.x, l.location.y)
        })
        .collect()
}
#[test]
fn asymmetric_margins_match_grid() {
    for last in [false, true] {
        let actual = positions(true, last);
        let reference = positions(false, last);
        assert_eq!(actual, reference);
    }
}
