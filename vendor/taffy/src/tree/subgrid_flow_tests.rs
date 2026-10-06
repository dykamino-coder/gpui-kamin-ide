//! Opposite grid flows preserve physical track sizes, padding edges and nested contributions.
use crate::prelude::*;
use crate::{AlignContent, Direction, GridPlacement};

fn line(start: i16, span: u16) -> Line<GridPlacement> {
    Line {
        start: GridPlacement::Line(start.into()),
        end: GridPlacement::Span(span),
    }
}

#[test]
fn inherited_tracks_preserve_asymmetric_physical_edges() {
    for parent_reverse in [false, true] {
        for child_reverse in [false, true] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
            tree.disable_rounding();
            let leaf = tree
                .new_leaf(Style {
                    grid_column: line(if child_reverse { 2 } else { 1 }, 1),
                    ..Style::DEFAULT
                })
                .unwrap();
            let subgrid = tree
                .new_with_children(
                    Style {
                        display: Display::Grid,
                        subgrid: crate::style::SUBGRID_COLUMNS,
                        grid_column: line(1, 2),
                        grid_axis_reversed: Some(Size {
                            width: child_reverse,
                            height: false,
                        }),
                        border: Rect {
                            left: length(3.0),
                            right: length(7.0),
                            top: length(0.0),
                            bottom: length(0.0),
                        },
                        gap: Size {
                            width: length(10.0),
                            height: length(0.0),
                        },
                        ..Style::DEFAULT
                    },
                    &[leaf],
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        display: Display::Grid,
                        size: Size {
                            width: length(116.0),
                            height: length(20.0),
                        },
                        padding: Rect {
                            left: length(5.0),
                            right: length(11.0),
                            top: length(0.0),
                            bottom: length(0.0),
                        },
                        grid_template_columns: vec![length(30.0), length(60.0)],
                        grid_axis_reversed: Some(Size {
                            width: parent_reverse,
                            height: false,
                        }),
                        gap: Size {
                            width: length(10.0),
                            height: length(0.0),
                        },
                        ..Style::DEFAULT
                    },
                    &[subgrid],
                )
                .unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let parent = tree.layout(subgrid).unwrap();
            let child = tree.layout(leaf).unwrap();
            assert_eq!(
                parent.location.x + child.location.x,
                8.0,
                "parent={parent_reverse} child={child_reverse}"
            );
            assert_eq!(child.size.width, if parent_reverse { 57.0 } else { 27.0 });
        }
    }
}

#[test]
fn nested_subgrid_row_contribution_geometry() {
    assert_eq!(geometry(false), vec![0.0, 0.0, 0.0, 50.0, 50.0, 40.0]);
    assert_eq!(geometry(true), vec![0.0, 0.0, 0.0, 50.0, 50.0, 40.0]);
}

fn geometry(mapped: bool) -> Vec<f32> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let markers = (1..=6)
        .map(|row| {
            tree.new_leaf(Style {
                min_size: Size {
                    width: length(10.0),
                    height: length(0.0),
                },
                grid_row: line(row, 1),
                ..Style::DEFAULT
            })
            .unwrap()
        })
        .collect::<Vec<_>>();
    let text = tree
        .new_leaf(Style {
            size: Size {
                width: length(70.0),
                height: length(100.0),
            },
            grid_row: line(2, 2),
            ..Style::DEFAULT
        })
        .unwrap();
    let inner = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                subgrid: crate::style::SUBGRID_ROWS,
                min_size: Size {
                    width: length(10.0),
                    height: length(0.0),
                },
                grid_template_columns: vec![auto()],
                // Clamp the requested five-track span before mapping the parent's reversed flow.
                grid_row: if mapped { line(1, 4) } else { line(2, 5) },
                grid_column: line(2, 5),
                border: Rect {
                    bottom: length(40.0),
                    top: length(0.0),
                    left: length(0.0),
                    right: length(0.0),
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
                direction: Direction::Rtl,
                grid_axis_reversed: Some(Size {
                    width: false,
                    height: !mapped,
                }),
                subgrid: crate::style::SUBGRID_ROWS,
                min_size: Size {
                    width: length(10.0),
                    height: length(0.0),
                },
                grid_template_columns: vec![auto()],
                grid_row: line(3, 5),
                grid_column: line(2, 5),
                border: Rect {
                    bottom: length(40.0),
                    top: length(0.0),
                    left: length(0.0),
                    right: length(0.0),
                },
                ..Style::DEFAULT
            },
            &[inner],
        )
        .unwrap();
    let mut children = markers.clone();
    children.push(outer);
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size {
                    width: length(784.0),
                    height: auto(),
                },
                grid_template_rows: vec![auto(); 4],
                grid_template_columns: vec![auto(); 5],
                align_content: Some(AlignContent::START),
                justify_content: Some(AlignContent::START),
                ..Style::DEFAULT
            },
            &children,
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(outer).unwrap().size.height, 180.0);
    assert_eq!(tree.layout(inner).unwrap().size.height, 140.0);
    assert_eq!(tree.layout(inner).unwrap().location.y, 0.0);
    assert_eq!(tree.layout(text).unwrap().location.y, 0.0);
    markers
        .into_iter()
        .map(|marker| tree.layout(marker).unwrap().size.height)
        .collect()
}
