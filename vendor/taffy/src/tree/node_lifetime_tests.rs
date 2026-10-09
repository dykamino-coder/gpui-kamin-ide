//! Teardown must release removed contexts while preserving live children and unrelated nodes.

use crate::TaffyTree;
use crate::Style;
use std::rc::Rc;

#[test]
fn clear_releases_every_context_before_the_tree_is_dropped() {
    let mut tree = TaffyTree::new();
    let first = Rc::new(1);
    let second = Rc::new(2);
    let first_weak = Rc::downgrade(&first);
    let second_weak = Rc::downgrade(&second);
    tree.new_leaf_with_context(Style::default(), first).unwrap();
    tree.new_leaf_with_context(Style::default(), second)
        .unwrap();
    tree.clear();
    assert!(first_weak.upgrade().is_none());
    assert!(second_weak.upgrade().is_none());
}

#[test]
fn remove_releases_only_the_removed_context_and_keeps_detached_children_live() {
    let mut tree = TaffyTree::new();
    let context = Rc::new(1);
    let weak = Rc::downgrade(&context);
    let child = tree
        .new_leaf_with_context(Style::default(), Rc::new(2))
        .unwrap();
    let unrelated = tree
        .new_leaf_with_context(Style::default(), Rc::new(3))
        .unwrap();
    let parent = tree.new_with_children(Style::default(), &[child]).unwrap();
    tree.set_node_context(parent, Some(context)).unwrap();
    tree.remove(parent).unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(**tree.get_node_context(child).unwrap(), 2);
    assert_eq!(**tree.get_node_context(unrelated).unwrap(), 3);
    assert!(tree.parent(child).is_none());
}
