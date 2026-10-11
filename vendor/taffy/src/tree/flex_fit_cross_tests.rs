//! Fit cross sizing honors inline contributions, grid children, and box edges.
use super::flex_intrinsic_cross_tests::column;
use crate::style_helpers::TaffyMaxContent;
use crate::{
    AvailableSpace, BoxSizing, Dimension, Display, FlexWrap, LengthPercentage, Rect, Size, Style,
    TaffyTree,
};

#[test]
fn fit_cross_limit_below_min_content_keeps_inline_item_floor() {
    for keyword in [
        Dimension::fit_content_px(50.0),
        Dimension::fit_content_percent(0.1),
    ] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let item = Style {
            size: Size::from_lengths(100.0, 0.0),
            flex_shrink: 0.0,
            ..Style::DEFAULT
        };
        let first = tree.new_leaf(item.clone()).unwrap();
        let second = tree.new_leaf(item).unwrap();
        let row = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_wrap: FlexWrap::Wrap,
                    percent_basis_from_parent: true,
                    ..Style::DEFAULT
                },
                &[first, second],
            )
            .unwrap();
        let green = tree
            .new_with_children(
                Style {
                    size: Size {
                        width: keyword,
                        height: Dimension::length(100.0),
                    },
                    ..column(keyword)
                },
                &[row],
            )
            .unwrap();
        let outer = tree
            .new_with_children(column(Dimension::length(200.0)), &[green])
            .unwrap();
        tree.compute_layout(outer, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.layout(green).unwrap().size,
            Size {
                width: 100.0,
                height: 100.0
            }
        );
    }
}

#[cfg(feature = "grid")]
#[test]
fn intrinsic_grid_cross_size_is_not_replaced_by_parent_stretch() {
    for outer_width in [300.0, 784.0, 900.0] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let leaf = tree
            .new_leaf(Style {
                size: Size::from_lengths(100.0, 100.0),
                ..Style::DEFAULT
            })
            .unwrap();
        let grid = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    size: Size {
                        width: Dimension::fit_content(),
                        height: Dimension::auto(),
                    },
                    ..Style::DEFAULT
                },
                &[leaf],
            )
            .unwrap();
        let outer = tree
            .new_with_children(column(Dimension::length(outer_width)), &[grid])
            .unwrap();
        tree.compute_layout(outer, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.layout(grid).unwrap().size,
            Size {
                width: 100.0,
                height: 100.0
            }
        );
    }
}

#[test]
fn flex_fit_cross_clamps_custom_inline_contributions_and_counts_edges_once() {
    for box_sizing in [BoxSizing::ContentBox, BoxSizing::BorderBox] {
        for (keyword, argument) in [
            (Dimension::fit_content_px(50.0), 50.0),
            (Dimension::fit_content_px(150.0), 150.0),
            (Dimension::fit_content_px(250.0), 250.0),
            (Dimension::fit_content_percent(0.5), 150.0),
        ] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
            tree.disable_rounding();
            let inline = tree.new_leaf(Style::DEFAULT).unwrap();
            let green = tree
                .new_with_children(
                    Style {
                        box_sizing,
                        padding: Rect {
                            left: LengthPercentage::length(5.0),
                            right: LengthPercentage::length(5.0),
                            ..Rect::zero()
                        },
                        border: Rect {
                            left: LengthPercentage::length(2.0),
                            right: LengthPercentage::length(2.0),
                            ..Rect::zero()
                        },
                        ..column(keyword)
                    },
                    &[inline],
                )
                .unwrap();
            let outer = tree
                .new_with_children(column(Dimension::length(300.0)), &[green])
                .unwrap();
            tree.compute_layout_with_measure(outer, Size::MAX_CONTENT, |inputs, _, _, style| {
                crate::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| Size {
                        width: known.width.unwrap_or(match available.width {
                            AvailableSpace::MinContent => 100.0,
                            AvailableSpace::MaxContent => 200.0,
                            AvailableSpace::Definite(value) => value,
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
                tree.layout(green).unwrap().size.width,
                target.clamp(114.0, 214.0)
            );
        }
    }
}
