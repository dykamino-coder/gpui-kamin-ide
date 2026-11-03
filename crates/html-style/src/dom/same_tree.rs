//! Value equality of box trees across document revisions and frames.
//!
//! Layout passes copy the nodes they rewrite (`Children` is copy-on-write), so
//! the same subtree of two frames is often a different allocation with the
//! same values. Shared child lists and style stages compare by pointer first;
//! the walk is iterative, as deep trees are legal input.

use super::{Element, Node};
use std::rc::Rc;

/// Whether two elements carry the same values apart from their children.
pub fn same_box(a: &Element, b: &Element) -> bool {
    let anim = match (&a.anim, &b.anim) {
        (None, None) => true,
        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
        _ => false,
    };
    let layer = |x: &Option<super::SharedStyle>, y: &Option<super::SharedStyle>| match (x, y) {
        (None, None) => true,
        (Some(x), Some(y)) => x.same(y),
        _ => false,
    };
    anim && a.node_id == b.node_id
        && a.list_item == b.list_item
        && a.inline == b.inline
        && a.tag == b.tag
        && a.marks == b.marks
        && a.attrs == b.attrs
        && a.style.same(&b.style)
        && layer(&a.hover, &b.hover)
        && layer(&a.first_letter, &b.first_letter)
        && layer(&a.first_line, &b.first_line)
}

/// Whether two subtrees carry the same values throughout.
pub fn same_tree(a: &Element, b: &Element) -> bool {
    let mut pending: Vec<(&Element, &Element)> = vec![(a, b)];
    while let Some((a, b)) = pending.pop() {
        if !same_box(a, b) {
            return false;
        }
        if a.children.identity() == b.children.identity() {
            continue;
        }
        if a.children.len() != b.children.len() {
            return false;
        }
        for pair in a.children.iter().zip(b.children.iter()) {
            match pair {
                (Node::Text(x), Node::Text(y)) => {
                    if x.as_str() != y.as_str() || x.source() != y.source() {
                        return false;
                    }
                }
                (Node::Element(x), Node::Element(y)) => pending.push((x, y)),
                _ => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{Marks, Text};
    use crate::style::computed::Computed;

    fn text(s: &str) -> Node {
        Node::Text(Text::from_source(s.into(), None))
    }

    fn block(children: Vec<Node>) -> Element {
        let mut e = Element::synthetic("div", Computed::default(), Marks::default(), children);
        e.node_id = 7;
        e
    }

    #[test]
    fn copies_with_equal_values_are_the_same_tree() {
        let a = block(vec![text("one"), Node::Element(block(vec![text("two")]))]);
        assert!(same_tree(&a, &a.clone()));
        let rebuilt = block(vec![text("one"), Node::Element(block(vec![text("two")]))]);
        assert!(same_tree(&a, &rebuilt));
    }

    #[test]
    fn a_changed_text_or_attribute_differs() {
        let a = block(vec![text("one")]);
        assert!(!same_tree(&a, &block(vec![text("onf")])));
        let mut b = a.clone();
        b.attrs.push(("class".into(), "x".into()));
        assert!(!same_tree(&a, &b));
        assert!(!same_tree(&a, &block(vec![text("one"), text("")])));
    }
}
