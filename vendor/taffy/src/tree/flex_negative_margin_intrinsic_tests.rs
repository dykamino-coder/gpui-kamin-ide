//! Negative sibling margins cannot invert wrapped intrinsic minimum and maximum.
use crate::prelude::*;
#[test]
fn wrapped_maximum_retains_minimum_with_negative_sibling_margin() {
    for direction in [FlexDirection::Row, FlexDirection::RowReverse] {
        for wrap in [FlexWrap::NoWrap, FlexWrap::Wrap] {
            for preferred in [Dimension::min_content(), Dimension::max_content()] {
                for margin in [-10.0, 0.0, 10.0] {
                    let mut tree: TaffyTree<()> = TaffyTree::new();
                    tree.disable_rounding();
                    let first = tree
                        .new_leaf(Style {
                            size: Size {
                                width: length(100.0),
                                height: Dimension::AUTO,
                            },
                            ..Style::DEFAULT
                        })
                        .unwrap();
                    let second = tree
                        .new_leaf(Style {
                            size: Size {
                                width: length(0.0),
                                height: Dimension::AUTO,
                            },
                            margin: Rect {
                                left: length(margin),
                                ..Rect::zero()
                            },
                            ..Style::DEFAULT
                        })
                        .unwrap();
                    let container = tree
                        .new_with_children(
                            Style {
                                display: Display::Flex,
                                flex_direction: direction,
                                flex_wrap: wrap,
                                size: Size {
                                    width: preferred,
                                    height: length(100.0),
                                },
                                ..Style::DEFAULT
                            },
                            &[first, second],
                        )
                        .unwrap();
                    let space = if preferred == Dimension::min_content() {
                        Size {
                            width: AvailableSpace::MinContent,
                            height: AvailableSpace::MaxContent,
                        }
                    } else {
                        Size::MAX_CONTENT
                    };
                    tree.compute_layout(container, space).unwrap();
                    let expected = if wrap == FlexWrap::Wrap {
                        if preferred == Dimension::min_content() {
                            100.0
                        } else {
                            (100.0_f32 + margin).max(100.0)
                        }
                    } else {
                        100.0 + margin
                    };
                    assert_eq!(
                        tree.layout(container).unwrap().size.width,
                        expected,
                        "{direction:?} {wrap:?} {preferred:?} {margin}"
                    );
                    assert_eq!(tree.layout(first).unwrap().size.width, 100.0);
                    assert_eq!(tree.layout(second).unwrap().size.width, 0.0);
                }
            }
        }
    }
}
