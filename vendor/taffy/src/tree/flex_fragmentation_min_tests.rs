//! KaminIDE patch: fragmentation preserves the used minimum of scroll items.
use crate::geometry::{Point, Size};
use crate::style_helpers::TaffyMaxContent;
use crate::{Dimension, Display, FlexDirection, LengthPercentageAuto, Overflow, Style, TaffyTree};

#[test]
fn fragmented_scroll_item_preserves_zero_automatic_minimum() {
    for overflow in [Overflow::Scroll, Overflow::Hidden] {
        for height in [Some(100.0), None] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
            tree.disable_rounding();
            let content = tree
                .new_leaf(Style {
                    size: Size::from_lengths(50.0, 200.0),
                    ..Style::DEFAULT
                })
                .unwrap();
            let item = tree
                .new_with_children(
                    Style {
                        display: Display::Block,
                        flex_basis: Dimension::length(0.0),
                        flex_grow: 1.0,
                        overflow: Point {
                            x: overflow,
                            y: overflow,
                        },
                        ..Style::DEFAULT
                    },
                    &[content],
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        flex_direction: FlexDirection::Column,
                        size: Size {
                            width: Dimension::length(50.0),
                            height: height.map_or(Dimension::auto(), Dimension::length),
                        },
                        ..Style::DEFAULT
                    },
                    &[item],
                )
                .unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let expected = tree.layout(item).unwrap().size.height;
            assert_eq!(expected, height.unwrap_or(0.0));
            tree.set_flex_fragmentation(root, true).unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            assert_eq!(
                tree.layout(item).unwrap().size.height,
                expected,
                "overflow={overflow:?}, container height={height:?}"
            );
        }
    }
}

#[test]
fn fragmented_monolithic_item_keeps_its_flex_size() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let content = tree
        .new_leaf(Style {
            size: Size::from_lengths(50.0, 200.0),
            ..Style::DEFAULT
        })
        .unwrap();
    let item = tree
        .new_with_children(
            Style {
                display: Display::Block,
                flex_basis: Dimension::length(10.0),
                min_size: Size {
                    width: LengthPercentageAuto::auto(),
                    height: LengthPercentageAuto::length(10.0),
                },
                ..Style::DEFAULT
            },
            &[content],
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: Dimension::length(50.0),
                    height: Dimension::auto(),
                },
                ..Style::DEFAULT
            },
            &[item],
        )
        .unwrap();
    tree.set_flex_fragmentation(root, true).unwrap();
    tree.set_fragmentation_monolithic(item, true).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(item).unwrap().size.height, 10.0);
    // Reclassification invalidates the parent result: a fragmentable box may grow.
    tree.set_fragmentation_monolithic(item, false).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(item).unwrap().size.height, 200.0);
}

#[test]
fn intrinsic_correction_respects_overflow_and_explicit_minimum() {
    for overflow in [
        Overflow::Visible,
        Overflow::Clip,
        Overflow::Scroll,
        Overflow::Hidden,
    ] {
        for height in [Some(100.0), None] {
            for minimum in [
                LengthPercentageAuto::auto(),
                LengthPercentageAuto::length(10.0),
            ] {
                let mut tree: TaffyTree<()> = TaffyTree::new();
                let content = tree
                    .new_leaf(Style {
                        size: Size::from_lengths(50.0, 200.0),
                        ..Style::DEFAULT
                    })
                    .unwrap();
                let item = tree
                    .new_with_children(
                        Style {
                            display: Display::Block,
                            flex_basis: Dimension::length(0.0),
                            flex_grow: 1.0,
                            min_size: Size {
                                width: LengthPercentageAuto::auto(),
                                height: minimum,
                            },
                            overflow: Point {
                                x: overflow,
                                y: overflow,
                            },
                            ..Style::DEFAULT
                        },
                        &[content],
                    )
                    .unwrap();
                let root = tree
                    .new_with_children(
                        Style {
                            flex_direction: FlexDirection::Column,
                            size: Size {
                                width: Dimension::length(50.0),
                                height: height.map_or(Dimension::auto(), Dimension::length),
                            },
                            ..Style::DEFAULT
                        },
                        &[item],
                    )
                    .unwrap();
                tree.set_flex_fragmentation(root, true).unwrap();
                tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                let expected = if overflow.is_scroll_container() {
                    height.unwrap_or(if minimum.is_auto() { 0.0 } else { 10.0 })
                } else if minimum.is_auto() || height.is_none() {
                    200.0
                } else {
                    100.0
                };
                assert_eq!(
                    tree.layout(item).unwrap().size.height,
                    expected,
                    "overflow={overflow:?}, height={height:?}, minimum={minimum:?}"
                );
            }
        }
    }
}
