//! Ratio percentage bases stay definite when content expands the used size.
use crate::style_helpers::{TaffyAuto, TaffyMaxContent};
use crate::{
    Dimension, Display, FlexDirection, FlexWrap, LengthPercentageAuto, Size, Style, TaffyTree,
};

fn block(preferred: Option<f32>, first_height: Option<f32>, minimum: f32) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let percentage = tree
        .new_leaf(Style {
            display: Display::Block,
            size: Size {
                width: Dimension::AUTO,
                height: Dimension::percent(1.0),
            },
            ..Style::DEFAULT
        })
        .unwrap();
    let mut children = vec![];
    if let Some(height) = first_height {
        children.push(
            tree.new_leaf(Style {
                display: Display::Block,
                size: Size {
                    width: Dimension::AUTO,
                    height: Dimension::length(height),
                },
                ..Style::DEFAULT
            })
            .unwrap(),
        );
    }
    children.push(percentage);
    let parent = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::AUTO,
                },
                min_size: Size {
                    width: LengthPercentageAuto::AUTO,
                    height: LengthPercentageAuto::length(minimum),
                },
                aspect_ratio_preferred_size: Size {
                    width: None,
                    height: preferred,
                },
                ..Style::DEFAULT
            },
            &children,
        )
        .unwrap();
    tree.compute_layout(parent, Size::MAX_CONTENT).unwrap();
    (
        tree.layout(parent).unwrap().size.height,
        tree.layout(percentage).unwrap().size.height,
    )
}

#[test]
fn ratio_transfer_resolves_child_percentages() {
    assert_eq!(block(Some(100.0), None, 100.0), (100.0, 100.0));
}

#[test]
fn expanded_content_does_not_replace_ratio_percentage_basis() {
    assert_eq!(block(Some(50.0), Some(50.0), 50.0), (100.0, 50.0));
}

#[test]
fn ordinary_minimum_does_not_make_percentages_definite() {
    assert_eq!(block(None, None, 100.0), (100.0, 0.0));
}

#[test]
fn ratio_preferred_column_length_wraps_without_fixing_auto_height() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let item = Style {
        size: Size {
            width: Dimension::length(50.0),
            height: Dimension::length(100.0),
        },
        ..Style::DEFAULT
    };
    let first = tree.new_leaf(item.clone()).unwrap();
    let second = tree.new_leaf(item).unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                flex_wrap: FlexWrap::Wrap,
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::AUTO,
                },
                min_size: Size {
                    width: LengthPercentageAuto::AUTO,
                    height: LengthPercentageAuto::length(100.0),
                },
                aspect_ratio_preferred_size: Size {
                    width: None,
                    height: Some(100.0),
                },
                ..Style::DEFAULT
            },
            &[first, second],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(root).unwrap().size.height, 100.0);
    assert_eq!(tree.layout(second).unwrap().location.x, 50.0);
    assert_eq!(tree.layout(second).unwrap().location.y, 0.0);
}
