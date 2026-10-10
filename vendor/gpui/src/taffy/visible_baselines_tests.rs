//! Intermediate lines are extracted from completed native leaf geometry with actual insets.

use super::*;
use crate::px;
use stacksafe::StackSafe;
use std::{cell::RefCell, rc::Rc};
use taffy::style_helpers::TaffyMaxContent;

#[test]
fn intermediate_line_metadata_includes_native_child_and_content_insets() {
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let context = NodeContext {
        measure: Rc::new(RefCell::new(StackSafe::new(Box::new(|_, _, _, _| {
            unreachable!("native fixture supplies its own measurement callback")
        })))),
        layout_lines: Some(vec![px(5.0), px(15.0), px(25.0)]),
        layout_lines_x: None,
        layout_lines_x_from_right: false,
    };
    let child = tree
        .new_leaf_with_context(
            Style {
                size: taffy::Size {
                    width: taffy::Dimension::length(30.0),
                    height: taffy::Dimension::length(36.0),
                },
                padding: taffy::Rect {
                    top: taffy::LengthPercentage::length(2.0),
                    left: taffy::LengthPercentage::length(0.0),
                    right: taffy::LengthPercentage::length(0.0),
                    bottom: taffy::LengthPercentage::length(0.0),
                },
                ..Style::default()
            },
            context,
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: taffy::Display::Block,
                size: taffy::Size {
                    width: taffy::Dimension::length(40.0),
                    height: taffy::Dimension::length(50.0),
                },
                padding: taffy::Rect {
                    top: taffy::LengthPercentage::length(4.0),
                    left: taffy::LengthPercentage::length(0.0),
                    right: taffy::LengthPercentage::length(0.0),
                    bottom: taffy::LengthPercentage::length(0.0),
                },
                ..Style::default()
            },
            &[child],
        )
        .unwrap();
    let positioned = tree
        .new_leaf_with_context(
            Style {
                position: taffy::Position::Absolute,
                size: taffy::Size {
                    width: taffy::Dimension::length(10.0),
                    height: taffy::Dimension::length(30.0),
                },
                ..Style::default()
            },
            NodeContext {
                measure: Rc::new(RefCell::new(StackSafe::new(Box::new(
                    |_, _, _, _| unreachable!(),
                )))),
                layout_lines: Some(vec![px(22.0)]),
                layout_lines_x: None,
                layout_lines_x_from_right: false,
            },
        )
        .unwrap();
    tree.add_child(root, positioned).unwrap();
    let mut floated_style = tree.style(positioned).unwrap().clone();
    floated_style.position = taffy::Position::Relative;
    floated_style.float = taffy::Float::Left;
    let floated = tree
        .new_leaf_with_context(
            floated_style,
            tree.get_node_context(positioned).unwrap().clone(),
        )
        .unwrap();
    tree.add_child(root, floated).unwrap();
    tree.compute_layout_with_measure(root, taffy::Size::MAX_CONTENT, |input, _, _, style| {
        taffy::compute_leaf_layout(input, style, taffy::tree::calc_value, |_, _| {
            taffy::MeasureOutput {
                size: taffy::Size {
                    width: 30.0,
                    height: 30.0,
                },
                baseline: Some(5.0),
                last_baseline: Some(25.0),
                baseline_x: None,
                last_baseline_x: None,
                baseline_x_from_right: false,
            }
        })
    })
    .unwrap();
    let mut lines = Vec::new();
    collect(&tree, root, 0.0, 1.0, &mut lines);
    assert_eq!(lines, vec![px(11.0), px(21.0), px(31.0)]);
    assert_eq!(
        lines
            .into_iter()
            .filter(|line| line.0 >= 16.0 && line.0 <= 26.0)
            .collect::<Vec<_>>(),
        vec![px(21.0)]
    );
    tree.mark_dirty(child).unwrap();
    let mut invalidated = Vec::new();
    collect(&tree, root, 0.0, 1.0, &mut invalidated);
    assert!(invalidated.is_empty());
}

#[test]
fn copied_line_metadata_is_independent_while_callbacks_are_shared() {
    let original = NodeContext {
        measure: Rc::new(RefCell::new(StackSafe::new(Box::new(
            |_, _, _, _| unreachable!(),
        )))),
        layout_lines: Some(vec![px(5.0), px(15.0)]),
        layout_lines_x: Some(vec![px(23.0), px(11.0)]),
        layout_lines_x_from_right: false,
    };
    let mut copy = original.clone();
    copy.layout_lines.as_mut().unwrap()[0] = px(7.0);
    copy.layout_lines_x.as_mut().unwrap()[0] = px(9.0);
    assert_eq!(original.layout_lines.as_ref().unwrap()[0], px(5.0));
    assert_eq!(original.layout_lines_x.as_ref().unwrap()[0], px(23.0));
    assert!(Rc::ptr_eq(&original.measure, &copy.measure));
}
