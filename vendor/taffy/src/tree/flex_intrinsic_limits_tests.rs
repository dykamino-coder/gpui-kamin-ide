//! Intrinsic flex minima win over smaller maxima without changing the basis.
use crate::prelude::*;

#[test]
fn explicit_intrinsic_minimum_overrides_maximum_in_each_axis() {
    for dir in [FlexDirection::Row, FlexDirection::Column] {
        for (keyword, row_minimum) in [
            (AvailableSpace::MinContent, 100.0),
            (AvailableSpace::MaxContent, 300.0),
        ] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
            let item = tree
                .new_leaf(Style {
                    flex_basis: Dimension::length(40.0),
                    item_min_size_keywords: if dir == FlexDirection::Row {
                        Size {
                            width: Some(keyword),
                            height: None,
                        }
                    } else {
                        Size {
                            width: None,
                            height: Some(keyword),
                        }
                    },
                    max_size: Size {
                        width: length(50.0),
                        height: length(50.0),
                    },
                    ..Style::DEFAULT
                })
                .unwrap();
            let parent = tree
                .new_with_children(
                    Style {
                        display: Display::Flex,
                        flex_direction: dir,
                        size: Size {
                            width: Dimension::length(500.0),
                            height: Dimension::length(500.0),
                        },
                        align_items: AlignItems::FLEX_START,
                        ..Style::DEFAULT
                    },
                    &[item],
                )
                .unwrap();
            tree.compute_layout_with_measure(parent, Size::MAX_CONTENT, |inputs, _, _, style| {
                crate::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| Size {
                        width: known.width.unwrap_or(match available.width {
                            AvailableSpace::MinContent => 100.0,
                            _ => 300.0,
                        }),
                        height: known.height.unwrap_or(200.0),
                    },
                )
            })
            .unwrap();
            let size = tree.layout(item).unwrap().size;
            assert_eq!(
                if dir == FlexDirection::Row {
                    size.width
                } else {
                    size.height
                },
                if dir == FlexDirection::Row {
                    row_minimum
                } else {
                    200.0
                }
            );
        }
    }
}
