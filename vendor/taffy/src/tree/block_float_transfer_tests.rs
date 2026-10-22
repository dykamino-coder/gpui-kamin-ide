//! Относительное смещение floats и stretch при примыкающих краях блока.
use crate::prelude::*;
use crate::{Direction, Float, Overflow, Point};

#[test]
fn relative_float_uses_parent_content_size_and_keeps_later_exclusions() {
    for direction in [Direction::Ltr, Direction::Rtl] {
        for definite_height in [false, true] {
            let mut tree: TaffyTree = TaffyTree::new();
            tree.disable_rounding();
            let first = tree
                .new_leaf(Style {
                    float: Float::Left,
                    position: Position::Relative,
                    size: Size::from_lengths(40.0, 30.0),
                    ..Style::DEFAULT
                })
                .unwrap();
            let second = tree
                .new_leaf(Style {
                    float: Float::Left,
                    size: Size::from_lengths(40.0, 30.0),
                    ..Style::DEFAULT
                })
                .unwrap();
            let block = tree
                .new_with_children(
                    Style {
                        display: Display::Block,
                        direction,
                        size: Size {
                            width: length(200.0),
                            height: if definite_height {
                                length(100.0)
                            } else {
                                auto()
                            },
                        },
                        padding: Rect {
                            left: length(10.0),
                            right: length(10.0),
                            top: length(10.0),
                            bottom: length(10.0),
                        },
                        ..Style::DEFAULT
                    },
                    &[first, second],
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        display: Display::Block,
                        size: Size::from_lengths(500.0, 300.0),
                        ..Style::DEFAULT
                    },
                    &[block],
                )
                .unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let normal_first = *tree.layout(first).unwrap();
            let normal_second = *tree.layout(second).unwrap();
            let mut style = tree.style(first).unwrap().clone();
            style.inset = Rect {
                left: percent(0.1),
                right: length(7.0),
                top: percent(0.25),
                bottom: length(3.0),
            };
            tree.set_style(first, style).unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let shifted = tree.layout(first).unwrap();
            assert_eq!(
                shifted.location.x - normal_first.location.x,
                if direction == Direction::Ltr {
                    18.0
                } else {
                    -7.0
                }
            );
            assert_eq!(
                shifted.location.y - normal_first.location.y,
                if definite_height { 20.0 } else { -3.0 }
            );
            assert_eq!(*tree.layout(second).unwrap(), normal_second);
        }
    }
}

#[test]
fn block_stretch_distinguishes_adjoining_edges_from_actual_margin_collapse() {
    for (padded, independent, floated, expected) in [
        (false, false, false, 100.0),
        (true, false, false, 65.0),
        (false, true, false, 70.0),
        (false, false, true, 70.0),
    ] {
        let mut tree: TaffyTree = TaffyTree::new();
        let child = tree
            .new_leaf(Style {
                display: Display::Block,
                float: if floated { Float::Left } else { Float::None },
                size: Size {
                    width: length(20.0),
                    height: Dimension::stretch(),
                },
                margin: Rect {
                    top: length(10.0),
                    bottom: length(20.0),
                    ..Rect::zero()
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let mut parent_style = Style {
            display: Display::Block,
            size: Size::from_lengths(200.0, 100.0),
            ..Style::DEFAULT
        };
        if padded {
            parent_style.padding.top = length(2.0);
            parent_style.padding.bottom = length(3.0);
        }
        if independent {
            parent_style.overflow = Point {
                x: Overflow::Hidden,
                y: Overflow::Hidden,
            };
        }
        let parent = tree.new_with_children(parent_style, &[child]).unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    ..Style::DEFAULT
                },
                &[parent],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.layout(child).unwrap().size.height,
            expected,
            "padded={padded}, independent={independent}, floated={floated}"
        );
    }
}
