//! Independent subtree measurement keeps native layout caches and callbacks separate.

use super::*;
use stacksafe::StackSafe;
use std::{cell::RefCell, rc::Rc};
use taffy::style_helpers::TaffyMaxContent;

fn fixed(width: f32, height: f32) -> Style {
    Style {
        size: taffy::Size {
            width: taffy::Dimension::length(width),
            height: taffy::Dimension::length(height),
        },
        ..Style::default()
    }
}

#[test]
fn copied_subtree_has_independent_styles_and_layout_cache() {
    let mut source = TaffyTree::<NodeContext>::new();
    source.disable_rounding();
    let child = source.new_leaf(fixed(12.5, 7.5)).unwrap();
    let root = source
        .new_with_children(fixed(40.0, 30.0), &[child])
        .unwrap();
    source
        .compute_layout(root, taffy::Size::MAX_CONTENT)
        .unwrap();
    let original = *source.layout(child).unwrap();
    let mut copy = TaffyTree::new();
    copy.disable_rounding();
    let copied_root = copy_subtree(&source, &mut copy, root);
    let copied_child = copy.children(copied_root).unwrap()[0];
    assert_eq!(
        copy.style(copied_child).unwrap(),
        source.style(child).unwrap()
    );
    assert!(copy.computed_layout_output(copied_root).unwrap().is_none());
    copy.set_style(copied_child, fixed(20.0, 10.0)).unwrap();
    copy.compute_layout(copied_root, taffy::Size::MAX_CONTENT)
        .unwrap();
    assert_eq!(copy.layout(copied_child).unwrap().size.width, 20.0);
    assert_eq!(*source.layout(child).unwrap(), original);
    assert_eq!(
        source.style(child).unwrap().size.width,
        taffy::Dimension::length(12.5)
    );
    assert!(source.computed_layout_output(root).unwrap().is_some());
}

#[test]
fn copied_subtree_keeps_actual_callback_alive_after_source_clear() {
    let mut source = TaffyTree::new();
    let context = NodeContext {
        measure: Rc::new(RefCell::new(StackSafe::new(Box::new(|_, _, _, _| {
            (
                crate::size(crate::px(12.0), crate::px(8.0)),
                None,
                None,
                None,
            ).into()
        })))),
        layout_lines: None,
        layout_lines_x: None,
        layout_lines_x_from_right: false,
    };
    let callback = Rc::downgrade(&context.measure);
    let root = source
        .new_leaf_with_context(fixed(12.0, 8.0), context)
        .unwrap();
    let mut copy = TaffyTree::new();
    let copied_root = copy_subtree(&source, &mut copy, root);
    assert!(Rc::ptr_eq(
        &source.get_node_context(root).unwrap().measure,
        &copy.get_node_context(copied_root).unwrap().measure,
    ));
    source.clear();
    assert!(callback.upgrade().is_some());
    copy.clear();
    assert!(callback.upgrade().is_none());
}
