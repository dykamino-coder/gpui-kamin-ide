//! KaminIDE patch: fragmentation consumes the physical block axis, independently of flex axes.
use crate::geometry::{AbsoluteAxis, Size};
use crate::style_helpers::TaffyMaxContent;
use crate::{AlignContent, Dimension, Direction, FlexDirection, FlexWrap, Style, TaffyTree};

#[test]
fn main_axis_offsets_preserve_native_sizes_in_both_block_directions() {
    for direction in [FlexDirection::Row, FlexDirection::RowReverse] {
        for layout_direction in [Direction::Ltr, Direction::Rtl] {
            for reversed in [false, true] {
                for auto in [false, true] {
                    let mut tree: TaffyTree<()> = TaffyTree::new();
                    tree.disable_rounding();
                    let child = Style {
                        size: Size::from_lengths(30.0, 40.0),
                        flex_shrink: 0.0,
                        ..Style::DEFAULT
                    };
                    let a = tree.new_leaf(child.clone()).unwrap();
                    let b = tree.new_leaf(child).unwrap();
                    let root = tree
                        .new_with_children(
                            Style {
                                flex_direction: direction,
                                direction: layout_direction,
                                size: Size {
                                    width: if auto {
                                        Dimension::auto()
                                    } else {
                                        Dimension::length(60.0)
                                    },
                                    height: Dimension::length(40.0),
                                },
                                ..Style::DEFAULT
                            },
                            &[a, b],
                        )
                        .unwrap();
                    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                    let before = [*tree.layout(a).unwrap(), *tree.layout(b).unwrap()];
                    tree.set_flex_fragmentation_axis(root, AbsoluteAxis::Horizontal, reversed)
                        .unwrap();
                    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                    assert_eq!(*tree.layout(a).unwrap(), before[0]);
                    assert_eq!(*tree.layout(b).unwrap(), before[1]);
                    let target = if (before[0].location.x < before[1].location.x) == reversed {
                        a
                    } else {
                        b
                    };
                    let target_x = tree.layout(target).unwrap().location.x;
                    tree.set_flex_item_fragmentation_offset(target, 50.0)
                        .unwrap();
                    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                    for (node, old) in [a, b].into_iter().zip(before) {
                        let consumed = if reversed {
                            old.location.x <= target_x
                        } else {
                            old.location.x >= target_x
                        };
                        let expected = old.location.x
                            + if reversed {
                                (if auto { 50.0 } else { 0.0 }) - if consumed { 50.0 } else { 0.0 }
                            } else if consumed {
                                50.0
                            } else {
                                0.0
                            };
                        let now = tree.layout(node).unwrap();
                        assert_eq!(
                            now.location.x, expected,
                            "{direction:?} {layout_direction:?} reversed={reversed} auto={auto}"
                        );
                        assert_eq!(now.location.y, old.location.y);
                        assert_eq!(now.size, old.size);
                    }
                    assert_eq!(
                        tree.layout(root).unwrap().size.width,
                        if auto { 110.0 } else { 60.0 }
                    );
                    tree.set_flex_item_fragmentation_offset(target, 0.0)
                        .unwrap();
                    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                    assert_eq!(*tree.layout(a).unwrap(), before[0]);
                    assert_eq!(*tree.layout(b).unwrap(), before[1]);
                }
            }
        }
    }
}

#[test]
fn cross_axis_growth_preserves_items_and_native_line_membership() {
    for direction in [FlexDirection::Column, FlexDirection::ColumnReverse] {
        for layout_direction in [Direction::Ltr, Direction::Rtl] {
            for wrap in [FlexWrap::Wrap, FlexWrap::WrapReverse] {
                for reversed in [false, true] {
                    for (auto, align) in [(true, AlignContent::START), (false, AlignContent::START), (false, AlignContent::END), (false, AlignContent::CENTER), (false, AlignContent::SPACE_BETWEEN)] {
                        let mut tree: TaffyTree<()> = TaffyTree::new();
                        tree.disable_rounding();
                        let item = Style {
                            size: Size::from_lengths(30.0, 40.0),
                            flex_shrink: 0.0,
                            ..Style::DEFAULT
                        };
                        let a = tree.new_leaf(item.clone()).unwrap();
                        let b = tree.new_leaf(item).unwrap();
                        let root = tree
                            .new_with_children(
                                Style {
                                    flex_direction: direction,
                                    direction: layout_direction,
                                    flex_wrap: wrap,
                                    align_content: align,
                                    size: Size {
                                        width: if auto {
                                            Dimension::auto()
                                        } else {
                                            Dimension::length(100.0)
                                        },
                                        height: Dimension::length(40.0),
                                    },
                                    ..Style::DEFAULT
                                },
                                &[a, b],
                            )
                            .unwrap();
                        tree.set_flex_fragmentation_axis(root, AbsoluteAxis::Horizontal, reversed)
                            .unwrap();
                        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                        let before = [*tree.layout(a).unwrap(), *tree.layout(b).unwrap()];
                        let crate::DetailedLayoutInfo::Flex(info) = tree.detailed_layout_info(root)
                        else {
                            panic!("no lines")
                        };
                        let members: Vec<_> = info.lines.iter().map(|l| l.items[0].node).collect();
                        let first = if (info.lines[0].cross_offset < info.lines[1].cross_offset)
                            == reversed
                        {
                            1
                        } else {
                            0
                        };
                        let first_node = members[first];
                        let mut growth = vec![0.0; 2];
                        growth[first] = 50.0;
                        tree.set_flex_line_fragmentation_growth(root, growth)
                            .unwrap();
                        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                        for (node, old) in [a, b].into_iter().zip(before) {
                            let shift = if node == first_node {
                                0.0
                            } else if reversed {
                                -50.0
                            } else {
                                50.0
                            };
                            let expected =
                                old.location.x + shift + if reversed && auto { 50.0 } else { 0.0 };
                            let now = tree.layout(node).unwrap();
                            assert_eq!(now.location.x, expected, "{direction:?} {layout_direction:?} {wrap:?} {align:?} reversed={reversed} auto={auto}");
                            assert_eq!(now.location.y, old.location.y);
                            assert_eq!(now.size, old.size);
                        }
                        assert_eq!(
                            tree.layout(root).unwrap().size.width,
                            if auto { 110.0 } else { 100.0 }
                        );
                        let crate::DetailedLayoutInfo::Flex(info) = tree.detailed_layout_info(root)
                        else {
                            panic!("no lines")
                        };
                        assert_eq!(
                            info.lines
                                .iter()
                                .map(|l| l.items[0].node)
                                .collect::<Vec<_>>(),
                            members
                        );
                    }
                }
            }
        }
    }
}
