//! KaminIDE patch: строки принадлежат финальному solver-проходу, включая повторную раскладку.

use crate::geometry::{Point, Rect, Size};
use crate::style_helpers::TaffyMaxContent;
use crate::tree::{DetailedFlexInfo, DetailedLayoutInfo, NodeId};
use crate::{AlignContent, AvailableSpace, Dimension, Direction, Display, FlexDirection, FlexWrap};
use crate::{LengthPercentageAuto, Position, Style, TaffyTree};

fn info(tree: &TaffyTree<()>, root: NodeId) -> &DetailedFlexInfo {
    match tree.detailed_layout_info(root) {
        DetailedLayoutInfo::Flex(info) => info,
        other => panic!("missing flex geometry: {other:?}"),
    }
}

fn leaf(tree: &mut TaffyTree<()>, w: f32, h: f32) -> NodeId {
    tree.new_leaf(Style {
        size: Size::from_lengths(w, h),
        flex_shrink: 0.0,
        ..Style::DEFAULT
    })
    .unwrap()
}

#[test]
fn real_lines_are_replaced_after_width_change_and_child_growth() {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let a = leaf(&mut tree, 40.0, 20.0);
    let b = leaf(&mut tree, 40.0, 30.0);
    let c = leaf(&mut tree, 40.0, 10.0);
    let root = tree
        .new_with_children(
            Style {
                size: Size {
                    width: Dimension::length(80.0),
                    height: Dimension::auto(),
                },
                flex_wrap: FlexWrap::Wrap,
                ..Style::DEFAULT
            },
            &[a, b, c],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let lines = &info(&tree, root).lines;
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].items.iter().map(|i| i.node).collect::<Vec<_>>(),
        [a, b]
    );
    assert_eq!((lines[0].cross_offset, lines[0].cross_size), (0.0, 30.0));
    assert_eq!(lines[1].cross_offset, 30.0);
    let mut style = tree.style(root).unwrap().clone();
    style.size.width = Dimension::length(40.0);
    tree.set_style(root, style).unwrap();
    let mut style = tree.style(b).unwrap().clone();
    style.size.height = Dimension::length(50.0);
    tree.set_style(b, style).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let lines = &info(&tree, root).lines;
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines.iter().map(|l| l.cross_offset).collect::<Vec<_>>(),
        [0.0, 20.0, 70.0]
    );
    assert_eq!(lines[1].items[0].cross_size, 50.0);
}

#[test]
fn reverse_rtl_and_align_content_export_physical_final_geometry() {
    for direction in [
        FlexDirection::Row,
        FlexDirection::RowReverse,
        FlexDirection::Column,
        FlexDirection::ColumnReverse,
    ] {
        for rtl in [Direction::Ltr, Direction::Rtl] {
            for wrap in [FlexWrap::Wrap, FlexWrap::WrapReverse] {
                let mut tree = TaffyTree::new();
                tree.disable_rounding();
                let a = leaf(&mut tree, 40.0, 40.0);
                let b = leaf(&mut tree, 40.0, 40.0);
                let root = tree
                    .new_with_children(
                        Style {
                            direction: rtl,
                            flex_direction: direction,
                            flex_wrap: wrap,
                            align_content: AlignContent::SPACE_BETWEEN,
                            size: if direction.is_row() {
                                Size::from_lengths(50.0, 120.0)
                            } else {
                                Size::from_lengths(120.0, 50.0)
                            },
                            ..Style::DEFAULT
                        },
                        &[a, b],
                    )
                    .unwrap();
                tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
                let data = info(&tree, root);
                assert_eq!(data.direction, direction);
                assert_eq!(data.lines.len(), 2);
                for line in &data.lines {
                    let item = &line.items[0];
                    let layout = tree.layout(item.node).unwrap();
                    let (main, cross) = if direction.is_row() {
                        (layout.location.x, layout.location.y)
                    } else {
                        (layout.location.y, layout.location.x)
                    };
                    assert_eq!((item.main_offset, item.cross_offset), (main, cross));
                    assert_eq!(line.cross_offset, cross);
                    assert_eq!(line.cross_size, 40.0);
                }
            }
        }
    }
}

#[test]
fn hidden_and_oof_are_absent_and_relative_insets_do_not_move_flow() {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let hidden = tree
        .new_leaf(Style {
            display: Display::None,
            ..Style::DEFAULT
        })
        .unwrap();
    let absolute = tree
        .new_leaf(Style {
            position: Position::Absolute,
            ..Style::DEFAULT
        })
        .unwrap();
    let relative = tree
        .new_leaf(Style {
            size: Size::from_lengths(30.0, 20.0),
            inset: Rect {
                left: LengthPercentageAuto::length(9.0),
                top: LengthPercentageAuto::length(7.0),
                ..Rect::auto()
            },
            ..Style::DEFAULT
        })
        .unwrap();
    let root = tree
        .new_with_children(Style::DEFAULT, &[hidden, absolute, relative])
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let items = &info(&tree, root).lines[0].items;
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].node, items[0].order), (relative, 2));
    assert_eq!((items[0].main_offset, items[0].cross_offset), (0.0, 0.0));
    assert_eq!(
        tree.layout(relative).unwrap().location,
        Point { x: 9.0, y: 7.0 }
    );
}

#[test]
fn intrinsic_height_recomputed_at_new_fragmentainer_width() {
    let mut tree = TaffyTree::new();
    let item = tree.new_leaf(Style::DEFAULT).unwrap();
    let root = tree.new_with_children(Style::DEFAULT, &[item]).unwrap();
    for (width, height) in [(100.0, 20.0), (50.0, 40.0)] {
        tree.mark_dirty(root).unwrap();
        tree.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::MaxContent,
            },
            |inputs, _, _, style| {
                crate::compute_leaf_layout(
                    inputs,
                    style,
                    crate::tree::calc_value,
                    |known, available| {
                        let w = known
                            .width
                            .unwrap_or_else(|| available.width.into_option().unwrap_or(100.0));
                        Size {
                            width: w,
                            height: known.height.unwrap_or(2000.0 / w),
                        }
                    },
                )
            },
        )
        .unwrap();
        let line = &info(&tree, root).lines[0];
        assert_eq!(line.cross_size, height);
        assert_eq!(line.items[0].cross_size, height);
    }
}
