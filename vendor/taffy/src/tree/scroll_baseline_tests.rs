//! A missing inline-block baseline synthesizes its margin edge; real scroll baselines clamp.
use crate::style_helpers::TaffyMaxContent;
use crate::tree::MeasureOutput;
use crate::{
    AlignItems, Display, FlexDirection, LengthPercentageAuto, Overflow, Rect, Size, Style,
    TaffyTree,
};

fn sibling_y(inline_block: bool) -> f32 {
    let mut tree: TaffyTree<f32> = TaffyTree::new();
    tree.disable_rounding();
    let content = tree
        .new_leaf_with_context(
            Style {
                size: Size::from_lengths(20.0, 20.0),
                ..Style::DEFAULT
            },
            10.0,
        )
        .unwrap();
    let atom = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size::from_lengths(30.0, 50.0),
                overflow: crate::Point {
                    x: Overflow::Hidden,
                    y: Overflow::Hidden,
                },
                margin: Rect {
                    bottom: LengthPercentageAuto::length(3.0),
                    top: LengthPercentageAuto::length(0.0),
                    left: LengthPercentageAuto::length(0.0),
                    right: LengthPercentageAuto::length(0.0),
                },
                baseline_from_last: inline_block,
                ..Style::DEFAULT
            },
            &[content],
        )
        .unwrap();
    let sibling = tree
        .new_leaf_with_context(
            Style {
                size: Size::from_lengths(20.0, 20.0),
                ..Style::DEFAULT
            },
            10.0,
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                align_items: Some(AlignItems::BASELINE),
                ..Style::DEFAULT
            },
            &[atom, sibling],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, context, style| {
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |_, _| MeasureOutput {
                size: Size {
                    width: 20.0,
                    height: 20.0,
                },
                baseline: context.as_deref().copied(),
                last_baseline: context.as_deref().copied(),
                baseline_x: None,
                last_baseline_x: None,
                baseline_x_from_right: false,
            },
        )
    })
    .unwrap();
    tree.layout(sibling).unwrap().location.y
}

#[test]
fn clipped_inline_block_uses_bottom_margin_edge() {
    assert_eq!(sibling_y(true), 43.0);
}
#[test]
fn ordinary_scrollable_flex_uses_its_content_baseline() {
    assert_eq!(sibling_y(false), 0.0);
}
