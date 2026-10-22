//! Внешние margins нулевых flex-элементов учитываются один раз во вкладе.
use crate::prelude::*;

#[test]
fn zero_content_margins_do_not_become_flexible_content() {
    for direction in [
        FlexDirection::Row,
        FlexDirection::RowReverse,
        FlexDirection::Column,
        FlexDirection::ColumnReverse,
    ] {
        for wrap in [FlexWrap::NoWrap, FlexWrap::Wrap] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
            tree.disable_rounding();
            let mut children = Vec::new();
            for (margin, grow) in [(10.0, 2.0), (20.0, 0.5)] {
                let mut style = Style {
                    size: Size::zero(),
                    flex_basis: length(0.0),
                    flex_grow: grow,
                    min_size: Size::zero(),
                    ..Style::DEFAULT
                };
                if direction.is_row() {
                    style.margin.left = length(margin);
                } else {
                    style.margin.top = length(margin);
                }
                children.push(tree.new_leaf(style).unwrap());
            }
            let mut style = Style {
                flex_direction: direction,
                flex_wrap: wrap,
                ..Style::DEFAULT
            };
            if direction.is_row() {
                style.size.width = Dimension::max_content();
            } else {
                style.size.height = Dimension::max_content();
            }
            let root = tree.new_with_children(style, &children).unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let size = tree.layout(root).unwrap().size;
            assert_eq!(
                if direction.is_row() {
                    size.width
                } else {
                    size.height
                },
                30.0,
                "{direction:?} {wrap:?}"
            );
            for child in children {
                let size = tree.layout(child).unwrap().size;
                assert_eq!(
                    if direction.is_row() {
                        size.width
                    } else {
                        size.height
                    },
                    0.0
                );
            }
        }
    }
}

#[test]
fn wrapped_intrinsic_content_floor_can_exceed_flex_basis() {
    for direction in [FlexDirection::Row, FlexDirection::RowReverse] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let content = tree
            .new_leaf_with_context(
                Style {
                    flex_basis: length(0.0),
                    flex_grow: 1.0,
                    min_size: Size::zero(),
                    ..Style::DEFAULT
                },
                (),
            )
            .unwrap();
        let sibling = tree
            .new_leaf(Style {
                size: Size::zero(),
                margin: Rect {
                    left: length(-10.0),
                    ..Rect::zero()
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    flex_direction: direction,
                    flex_wrap: FlexWrap::Wrap,
                    size: Size {
                        width: Dimension::max_content(),
                        height: length(20.0),
                    },
                    ..Style::DEFAULT
                },
                &[content, sibling],
            )
            .unwrap();
        tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
            crate::compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |_, _| Size {
                    width: 100.0,
                    height: 10.0,
                },
            )
        })
        .unwrap();
        assert_eq!(
            tree.layout(root).unwrap().size.width,
            100.0,
            "{direction:?}"
        );
    }
}
