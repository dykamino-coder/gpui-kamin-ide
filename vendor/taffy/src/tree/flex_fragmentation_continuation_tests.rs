//! KaminIDE patch: consumed space follows block-start without changing flexible lengths.
use crate::geometry::{AbsoluteAxis, Size};
use crate::style_helpers::TaffyMaxContent;
use crate::{DetailedLayoutInfo, Dimension, Direction, FlexDirection, Position, Style, TaffyTree};

#[test]
fn horizontal_cross_axis_continuations_keep_original_item_sizes() {
    for direction in [Direction::Ltr, Direction::Rtl] {
        for reversed in [false, true] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
            tree.disable_rounding();
            let item = Style {
                size: Size::from_lengths(30.0, 40.0),
                ..Style::DEFAULT
            };
            let a = tree.new_leaf(item.clone()).unwrap();
            let b = tree.new_leaf(item).unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        flex_direction: FlexDirection::Column,
                        direction,
                        size: Size {
                            width: Dimension::auto(),
                            height: Dimension::length(80.0),
                        },
                        ..Style::DEFAULT
                    },
                    &[a, b],
                )
                .unwrap();
            tree.set_flex_fragmentation_axis(root, AbsoluteAxis::Horizontal, reversed)
                .unwrap();
            tree.set_flex_item_fragmentation_consumed(a, 20.0).unwrap();
            tree.set_flex_item_fragmentation_consumed(b, 10.0).unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            assert_eq!(tree.layout(root).unwrap().size.width, 20.0);
            for (node, consumed) in [(a, 20.0), (b, 10.0)] {
                let actual = tree.layout(node).unwrap();
                assert_eq!(actual.size, Size { width: 30.0, height: 40.0 });
                assert_eq!(
                    actual.location.x,
                    if reversed { consumed - 10.0 } else { -consumed },
                    "{direction:?} reversed={reversed}"
                );
            }
            let DetailedLayoutInfo::Flex(info) = tree.detailed_layout_info(root) else {
                panic!("no lines")
            };
            assert_eq!(info.lines[0].cross_offset, 0.0);
            assert_eq!(info.lines[0].cross_size, 20.0);
            tree.set_flex_item_fragmentation_consumed(a, 0.0).unwrap();
            tree.set_flex_item_fragmentation_consumed(b, 0.0).unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            assert_eq!(tree.layout(root).unwrap().size.width, 30.0);
        }
    }
}

#[test]
fn horizontal_main_axis_offset_keeps_flex_grow_and_absolute_static_position() {
    for reversed in [false, true] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let child = Style {
            size: Size::from_lengths(30.0, 40.0),
            flex_grow: 1.0,
            ..Style::DEFAULT
        };
        let a = tree.new_leaf(child.clone()).unwrap();
        let b = tree.new_leaf(child).unwrap();
        let abs = tree
            .new_leaf(Style {
                position: Position::Absolute,
                size: Size::from_lengths(10.0, 10.0),
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    size: Size::from_lengths(100.0, 40.0),
                    ..Style::DEFAULT
                },
                &[a, b, abs],
            )
            .unwrap();
        tree.set_flex_fragmentation_axis(root, AbsoluteAxis::Horizontal, reversed)
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let before = [*tree.layout(a).unwrap(), *tree.layout(b).unwrap()];
        let static_position = *tree.layout(abs).unwrap();
        tree.set_flex_item_fragmentation_offset(if reversed { a } else { b }, 50.0)
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(*tree.layout(abs).unwrap(), static_position);
        for (node, old) in [a, b].into_iter().zip(before) {
            assert_eq!(tree.layout(node).unwrap().size, old.size);
        }
        assert_eq!(
            tree.layout(a).unwrap().location.x,
            if reversed { -50.0 } else { 0.0 }
        );
        assert_eq!(
            tree.layout(b).unwrap().location.x,
            if reversed { 50.0 } else { 100.0 }
        );
        assert_eq!(tree.layout(root).unwrap().size.width, 100.0);
    }
}

#[test]
fn main_only_size_probes_include_consumed_space_without_publishing_layout() {
    use crate::{LayoutInput, LayoutPartialTree, RequestedAxis, RunMode};
    for (direction, axis) in [
        (FlexDirection::Row, AbsoluteAxis::Horizontal),
        (FlexDirection::Column, AbsoluteAxis::Vertical),
    ] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let child = Style {
            size: Size::from_lengths(30.0, 30.0),
            flex_shrink: 0.0,
            ..Style::DEFAULT
        };
        let a = tree.new_leaf(child.clone()).unwrap();
        let b = tree.new_leaf(child).unwrap();
        let root = tree
            .new_with_children(
                Style {
                    flex_direction: direction,
                    ..Style::DEFAULT
                },
                &[a, b],
            )
            .unwrap();
        tree.set_flex_fragmentation_axis(root, axis, false).unwrap();
        tree.set_flex_item_fragmentation_offset(b, 50.0).unwrap();
        let input = LayoutInput {
            run_mode: RunMode::ComputeSize,
            axis: RequestedAxis::from(axis),
            ..LayoutInput::HIDDEN
        };
        let probe = tree.as_layout_tree().compute_child_layout(root, input);
        assert_eq!(probe.size.main(direction), 110.0);
        assert!(matches!(
            tree.detailed_layout_info(root),
            DetailedLayoutInfo::None
        ));
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(
            tree.layout(root).unwrap().size.main(direction),
            probe.size.main(direction)
        );
        assert_eq!(tree.layout(b).unwrap().location.main(direction), 80.0);
    }
}
