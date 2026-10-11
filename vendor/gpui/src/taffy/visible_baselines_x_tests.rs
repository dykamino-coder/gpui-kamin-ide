//! The middle visible X line comes from scaled final geometry, not endpoint interpolation.

use super::*;
use crate::{MeasuredContent, px};
use stacksafe::StackSafe;
use std::{cell::RefCell, rc::Rc};
use taffy::style_helpers::TaffyMaxContent;

#[test]
fn clipped_right_origin_lines_include_final_width_insets_and_scrollbar_gutter() {
    for scroll in [false, true] {
        let mut tree = TaffyTree::new();
        tree.disable_rounding();
        let context = NodeContext {
            measure: Rc::new(RefCell::new(StackSafe::new(Box::new(|_, _, _, _| unreachable!())))),
            layout_lines: None,
            layout_lines_x: Some(vec![px(11.0), px(17.0), px(23.0)]),
            layout_lines_x_from_right: true,
        };
        let mut style: Style = Style::DEFAULT;
        style.size = taffy::Size { width: taffy::Dimension::length(100.0), height: taffy::Dimension::length(25.0) };
        style.padding.left = taffy::LengthPercentage::length(3.75);
        style.border.left = taffy::LengthPercentage::length(2.5);
        style.padding.right = taffy::LengthPercentage::length(6.25);
        style.border.right = taffy::LengthPercentage::length(1.25);
        style.scrollbar_width = 3.75;
        if scroll { style.overflow.y = taffy::Overflow::Scroll; }
        let node = tree.new_leaf_with_context(style, context).unwrap();
        tree.compute_layout_with_measure(node, taffy::Size::MAX_CONTENT, |input, _, _, style| {
            taffy::compute_leaf_layout(input, style, |_, _| 0.0, |_, _| MeasuredContent {
                first_x: Some(px(11.0)), last_x: Some(px(23.0)), x_from_right: true,
                ..MeasuredContent::new(size(px(40.0), px(20.0)))
            }.to_native(1.25))
        }).unwrap();
        let mut lines = Vec::new();
        collect(&tree, node, 0.0, 1.25, &mut lines);
        let gutter = if scroll { 3.0 } else { 0.0 };
        assert_eq!(lines, vec![px(63.0 - gutter), px(57.0 - gutter), px(51.0 - gutter)]);
        let expected = px(57.0 - gutter);
        assert_eq!(visible(lines, expected - px(1.0), px(2.0)), (Some(expected), Some(expected)));
        assert_eq!(tree.computed_layout_output(node).unwrap().unwrap().baselines.first, None);
    }
}
