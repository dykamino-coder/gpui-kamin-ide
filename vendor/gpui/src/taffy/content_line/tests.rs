//! Запрос готового дерева пропускает пустой блок и внепоточных детей.
use super::*;
use std::{cell::RefCell, rc::Rc};
use taffy::style_helpers::TaffyMaxContent;

#[test]
fn first_content_uses_later_line_without_remeasurement_and_rejects_dirty_tree() {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let context = || NodeContext {
        static_positions: Vec::new(),
        measure: Rc::new(RefCell::new(node_measure(Box::new(
            |_, _, _, _, _| unreachable!(),
        )))),
        layout_lines: None,
        layout_lines_x: None,
        layout_lines_x_from_right: false,
    };
    let empty = tree
        .new_leaf(taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::length(40.),
                height: taffy::Dimension::length(20.),
            },
            ..Default::default()
        })
        .unwrap();
    let absolute = tree
        .new_leaf_with_context(
            taffy::Style {
                position: taffy::Position::Absolute,
                ..Default::default()
            },
            context(),
        )
        .unwrap();
    let floated = tree
        .new_leaf_with_context(
            taffy::Style {
                float: taffy::Float::Left,
                ..Default::default()
            },
            context(),
        )
        .unwrap();
    let text = tree
        .new_leaf_with_context(Default::default(), context())
        .unwrap();
    let root = tree
        .new_with_children(
            taffy::Style {
                display: taffy::Display::Block,
                size: taffy::Size {
                    width: taffy::Dimension::length(100.),
                    height: taffy::Dimension::auto(),
                },
                ..Default::default()
            },
            &[empty, absolute, floated, text],
        )
        .unwrap();
    tree.compute_layout_with_measure(
        root,
        taffy::Size::MAX_CONTENT,
        |input, _, context, style| {
            let measured = context.is_some();
            taffy::compute_leaf_layout(input, style, taffy::tree::calc_value, |_, _| {
                taffy::MeasureOutput {
                    size: taffy::Size {
                        width: 10.,
                        height: 10.,
                    },
                    baseline: measured.then_some(7.),
                    last_baseline: measured.then_some(7.),
                    baseline_x: measured.then_some(4.),
                    last_baseline_x: measured.then_some(4.),
                    baseline_x_from_right: false,
                }
            })
        },
    )
    .unwrap();
    let y = tree.unrounded_layout(text).location.y + 7.;
    assert_eq!(first(&tree, root, crate::Axis::Vertical, 1.), Some(y));
    assert_eq!(first(&tree, root, crate::Axis::Vertical, 2.), Some(y / 2.));
    assert_eq!(
        first(&tree, root, crate::Axis::Horizontal, 1.),
        Some(tree.unrounded_layout(text).location.x + 4.)
    );
    tree.mark_dirty(text).unwrap();
    assert_eq!(first(&tree, root, crate::Axis::Vertical, 1.), None);
}
