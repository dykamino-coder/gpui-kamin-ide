//! Real flex layouts distinguish a projected logical cross-start from wrap reversal.
use crate::style_helpers::TaffyMaxContent;
use crate::{
    AlignItems, AlignmentSafety, Direction, FlexDirection, FlexWrap, Size, Style, TaffyTree,
};

fn position(projected: bool, wrap: FlexWrap, direction: Direction) -> f32 {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let child = tree
        .new_leaf(Style {
            size: Size::from_lengths(40.0, 30.0),
            flex_shrink: 0.0,
            align_self: Some(AlignItems {
                safety: AlignmentSafety::Safe,
                ..AlignItems::CENTER
            }),
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                size: Size::from_lengths(30.0, 30.0),
                flex_direction: FlexDirection::Column,
                flex_cross_reverse: projected,
                flex_wrap: wrap,
                direction,
                ..Style::DEFAULT
            },
            &[child],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    tree.layout(child).unwrap().location.x
}

#[test]
fn safe_overflow_uses_projected_rtl_column_start() {
    assert_eq!(position(true, FlexWrap::NoWrap, Direction::Ltr), -10.0);
}
#[test]
fn safe_overflow_uses_native_rtl_column_start() {
    assert_eq!(position(false, FlexWrap::NoWrap, Direction::Rtl), -10.0);
}
#[test]
fn wrapping_reversal_does_not_reverse_logical_start() {
    assert_eq!(position(false, FlexWrap::WrapReverse, Direction::Ltr), 0.0);
}
#[test]
fn ordinary_ltr_overflow_stays_at_logical_start() {
    assert_eq!(position(false, FlexWrap::NoWrap, Direction::Ltr), 0.0);
}
