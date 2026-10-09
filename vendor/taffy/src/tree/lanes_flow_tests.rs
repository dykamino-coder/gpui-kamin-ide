//! Physical stacking coordinates must compose writing direction, fill reversal and alignment.
use crate::prelude::*;
use crate::{AlignContent, AlignItems, GridLanes};

fn positions(rows: bool, reverse: bool, fill: bool, center: bool) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let margins = |start, end| {
        if rows {
            Rect {
                left: length(start),
                right: length(end),
                top: length(0.0),
                bottom: length(0.0),
            }
        } else {
            Rect {
                top: length(start),
                bottom: length(end),
                left: length(0.0),
                right: length(0.0),
            }
        }
    };
    let first = tree
        .new_leaf(Style {
            size: Size::from_lengths(20.0, 20.0),
            margin: margins(3.0, 5.0),
            position: Position::Relative,
            inset: if rows {
                Rect {
                    left: length(9.0),
                    ..Rect::auto()
                }
            } else {
                Rect {
                    top: length(9.0),
                    ..Rect::auto()
                }
            },
            ..Style::DEFAULT
        })
        .unwrap();
    let second = tree
        .new_leaf(Style {
            size: if rows {
                Size::from_lengths(30.0, 20.0)
            } else {
                Size::from_lengths(20.0, 30.0)
            },
            margin: margins(2.0, 6.0),
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_lanes: Some(GridLanes {
                    rows,
                    fill_reverse: fill,
                    track_reverse: false,
                    dense: false,
                    tolerance: 0.0,
                    tolerance_pct: None,
                    stack_block: false,
                }),
                grid_axis_reversed: Some(Size {
                    width: rows && reverse,
                    height: !rows && reverse,
                }),
                size: if rows {
                    Size::from_lengths(200.0, 80.0)
                } else {
                    Size::from_lengths(80.0, 200.0)
                },
                padding: if rows {
                    Rect {
                        left: length(7.0),
                        right: length(11.0),
                        top: length(0.0),
                        bottom: length(0.0),
                    }
                } else {
                    Rect {
                        top: length(7.0),
                        bottom: length(11.0),
                        left: length(0.0),
                        right: length(0.0),
                    }
                },
                gap: Size {
                    width: length(4.0),
                    height: length(4.0),
                },
                grid_template_columns: if rows { vec![] } else { vec![length(80.0)] },
                grid_template_rows: if rows { vec![length(80.0)] } else { vec![] },
                align_items: if rows {
                    AlignItems::START
                } else {
                    AlignItems::NORMAL
                },
                justify_items: if rows {
                    AlignItems::NORMAL
                } else {
                    AlignItems::START
                },
                align_content: if !rows && center {
                    AlignContent::CENTER
                } else {
                    AlignContent::NORMAL
                },
                justify_content: if rows && center {
                    AlignContent::CENTER
                } else {
                    AlignContent::NORMAL
                },
                ..Style::DEFAULT
            },
            &[first, second],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let a = tree.layout(first).unwrap().location;
    let b = tree.layout(second).unwrap().location;
    if rows {
        (a.x, b.x)
    } else {
        (a.y, b.y)
    }
}

#[test]
fn physical_stack_direction_composes_with_fill_and_asymmetric_edges() {
    for rows in [false, true] {
        for reverse in [false, true] {
            for fill in [false, true] {
                let expected = if reverse != fill {
                    (173.0, 121.0)
                } else {
                    (19.0, 41.0)
                };
                assert_eq!(
                    positions(rows, reverse, fill, false),
                    expected,
                    "rows={rows} reverse={reverse} fill={fill}"
                );
            }
        }
    }
}

#[test]
fn centered_stack_direction_preserves_physical_relative_offsets() {
    for rows in [false, true] {
        for reverse in [false, true] {
            for fill in [false, true] {
                let expected = if reverse != fill {
                    (117.0, 65.0)
                } else {
                    (75.0, 97.0)
                };
                assert_eq!(
                    positions(rows, reverse, fill, true),
                    expected,
                    "rows={rows} reverse={reverse} fill={fill}"
                );
            }
        }
    }
}
