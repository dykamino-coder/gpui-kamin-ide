//! Scrollbar geometry participates in track overflow and intrinsic ancestor sizing.
use super::*;
use crate::style::GridLanes;

#[test]
fn empty_tracks_export_overflow_without_children() {
    for lanes in [false, true] {
        for rows in [false, true] {
            for track in [40.0, 150.0] {
                for reverse in [false, true] {
                    let mut tree: TaffyTree<()> = TaffyTree::new();
                    let tracks = vec![length(track)];
                    let root = tree
                        .new_leaf(Style {
                            display: Display::Grid,
                            grid_lanes: lanes.then_some(GridLanes {
                                rows,
                                track_reverse: reverse,
                                fill_reverse: false,
                                dense: false,
                                tolerance: 0.0,
                                tolerance_pct: None,
                                stack_block: false,
                            }),
                            grid_template_columns: if rows { vec![] } else { tracks.clone() },
                            grid_template_rows: if rows { tracks } else { vec![] },
                            size: Size {
                                width: length(100.0),
                                height: length(100.0),
                            },
                            padding: Rect::length(3.0),
                            border: Rect::length(2.0),
                            overflow: Point {
                                x: Overflow::Hidden,
                                y: Overflow::Hidden,
                            },
                            scrollbar_width: BAR,
                            kamin_scrollbars: ScrollbarLayout::new(
                                Point { x: true, y: true },
                                false,
                                false,
                                None,
                            ),
                            ..Default::default()
                        })
                        .unwrap();
                    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                    let layout = tree.layout(root).unwrap();
                    let overflow = track > 90.0;
                    let expected = Size {
                        width: if overflow && rows { BAR } else { 0.0 },
                        height: if overflow && !rows { BAR } else { 0.0 },
                    };
                    assert_eq!(
                        layout.scrollbar_size, expected,
                        "lanes={lanes}, rows={rows}, track={track}, reverse={reverse}"
                    );
                }
            }
        }
    }
}

fn intrinsic_fixture(force_bar: bool) -> (TaffyTree<()>, NodeId, NodeId, NodeId, NodeId) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let child = tree
        .new_leaf(Style {
            size: Size {
                width: length(10.0),
                height: length(80.0),
            },
            flex_shrink: 0.0,
            ..Default::default()
        })
        .unwrap();
    let scroller = tree
        .new_with_children(
            Style {
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: auto(),
                    height: length(79.0),
                },
                overflow: Point {
                    x: Overflow::Hidden,
                    y: if force_bar {
                        Overflow::Scroll
                    } else {
                        Overflow::Hidden
                    },
                },
                scrollbar_width: BAR,
                kamin_scrollbars: ScrollbarLayout::new(
                    Point {
                        x: false,
                        y: !force_bar,
                    },
                    false,
                    false,
                    None,
                ),
                ..Default::default()
            },
            &[child],
        )
        .unwrap();
    let positioned = tree
        .new_with_children(
            Style {
                position: Position::Absolute,
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            &[scroller],
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: length(200.0),
                    height: length(200.0),
                },
                ..Default::default()
            },
            &[positioned],
        )
        .unwrap();
    (tree, root, positioned, scroller, child)
}

fn intrinsic_scroller(force_bar: bool) -> (f32, f32) {
    let (mut tree, root, positioned, scroller, _) = intrinsic_fixture(force_bar);
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    (
        tree.layout(positioned).unwrap().size.width,
        tree.layout(scroller).unwrap().scrollbar_size.width,
    )
}

#[test]
fn descendant_bar_updates_the_shrink_to_fit_ancestor() {
    let forced = intrinsic_scroller(true);
    let automatic = intrinsic_scroller(false);
    assert_eq!(forced, (25.0, BAR));
    assert_eq!(automatic, forced);
}

#[path = "kamin_scrollbars_retry_tests.rs"]
mod retry_tests;
