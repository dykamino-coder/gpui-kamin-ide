//! Fit-content must clamp between intrinsic contributions even when used layout overflows.
use crate::prelude::*;

#[test]
fn block_fit_content_clamps_a_custom_inline_formatting_context() {
    for (argument, expected) in [(50.0, 100.0), (150.0, 150.0), (250.0, 200.0)] {
        let mut tree: TaffyTree = TaffyTree::new();
        tree.disable_rounding();
        let inline = tree.new_leaf(Style::DEFAULT).unwrap();
        let block = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    size: Size {
                        width: Dimension::fit_content_px(argument),
                        height: auto(),
                    },
                    ..Style::DEFAULT
                },
                &[inline],
            )
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Block,
                    size: Size {
                        width: length(300.0),
                        height: auto(),
                    },
                    ..Style::DEFAULT
                },
                &[block],
            )
            .unwrap();
        tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, _, _, style| {
            crate::compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, available| Size {
                    width: known.width.unwrap_or(match available.width {
                        AvailableSpace::MinContent => 100.0,
                        AvailableSpace::MaxContent => 200.0,
                        AvailableSpace::Definite(v) => v,
                    }),
                    height: 20.0,
                },
            )
        })
        .unwrap();
        assert_eq!(
            tree.layout(block).unwrap().size.width,
            expected,
            "argument={argument}"
        );
    }
}

#[test]
fn block_fit_content_counts_edges_once_for_length_and_percentage_arguments() {
    for box_sizing in [BoxSizing::ContentBox, BoxSizing::BorderBox] {
        for (dimension, argument) in [
            (Dimension::fit_content_px(50.0), 50.0),
            (Dimension::fit_content_px(150.0), 150.0),
            (Dimension::fit_content_px(250.0), 250.0),
            (Dimension::fit_content_percent(0.5), 150.0),
        ] {
            let mut tree: TaffyTree = TaffyTree::new();
            tree.disable_rounding();
            let inline = tree.new_leaf(Style::DEFAULT).unwrap();
            let block = tree
                .new_with_children(
                    Style {
                        display: Display::Block,
                        box_sizing,
                        padding: Rect {
                            left: length(5.0),
                            right: length(5.0),
                            ..Rect::zero()
                        },
                        border: Rect {
                            left: length(2.0),
                            right: length(2.0),
                            ..Rect::zero()
                        },
                        size: Size {
                            width: dimension,
                            height: auto(),
                        },
                        ..Style::DEFAULT
                    },
                    &[inline],
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        display: Display::Block,
                        size: Size {
                            width: length(300.0),
                            height: auto(),
                        },
                        ..Style::DEFAULT
                    },
                    &[block],
                )
                .unwrap();
            tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, _, _, style| {
                crate::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| Size {
                        width: known.width.unwrap_or(match available.width {
                            AvailableSpace::MinContent => 100.0,
                            AvailableSpace::MaxContent => 200.0,
                            AvailableSpace::Definite(v) => v,
                        }),
                        height: 20.0,
                    },
                )
            })
            .unwrap();
            let target: f32 = argument
                + if box_sizing == BoxSizing::ContentBox {
                    14.0
                } else {
                    0.0
                };
            assert_eq!(
                tree.layout(block).unwrap().size.width,
                target.clamp(114.0, 214.0)
            );
        }
    }
}
