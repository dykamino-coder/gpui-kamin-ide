//! Partial parser trees drop without allocating or mutating shared descendants.

use crate::{append, Node, NodeData};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn retained_subtree_keeps_its_children() {
    let root = Node::new(NodeData::Document);
    let retained = Node::new(NodeData::Document);
    let leaf = Node::new(NodeData::Text {
        contents: RefCell::new("KEEP".into()),
    });
    append(&retained, leaf.clone());
    append(&root, retained.clone());
    drop(root);
    assert_eq!(retained.children.borrow().len(), 1);
    assert!(Rc::ptr_eq(&retained.children.borrow()[0], &leaf));
}

#[test]
fn partial_deep_and_wide_trees_drop_on_small_stack() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let root = Node::new(NodeData::Document);
            let mut parent = root.clone();
            for _ in 0..20_000 {
                let next = Node::new(NodeData::Document);
                append(&parent, Node::new(NodeData::Document));
                append(&parent, next.clone());
                parent = next;
            }
            drop(parent);
            drop(root);
        })
        .unwrap()
        .join()
        .unwrap();
}
