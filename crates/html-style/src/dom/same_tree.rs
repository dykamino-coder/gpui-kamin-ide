//! Value equality of box trees across document revisions and frames.
//!
//! Layout passes copy the nodes they rewrite (`Children` is copy-on-write), so
//! the same subtree of two frames is often a different allocation with the
//! same values. Shared child lists and style stages compare by pointer first;
//! the walk is iterative, as deep trees are legal input.

use super::{Children, Element, Node};
use std::rc::Rc;

/// Whether two elements carry the same values apart from their children.
pub fn same_box(a: &Element, b: &Element) -> bool {
    a.node_id == b.node_id && same_values(a, b)
}

/// Whether two elements carry the same values apart from their children and
/// their node ids.
pub fn same_values(a: &Element, b: &Element) -> bool {
    same_shape(a, b) && same_styles(a, b)
}

/// The values of two elements that compare in a few words: everything but
/// marks and style stages.
fn same_shape(a: &Element, b: &Element) -> bool {
    let anim = match (&a.anim, &b.anim) {
        (None, None) => true,
        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
        _ => false,
    };
    anim && a.list_item == b.list_item && a.inline == b.inline && a.tag == b.tag && a.attrs == b.attrs
}

/// Marks and style stages, which may compare field by field.
fn same_styles(a: &Element, b: &Element) -> bool {
    let layer = |x: &Option<super::SharedStyle>, y: &Option<super::SharedStyle>| match (x, y) {
        (None, None) => true,
        (Some(x), Some(y)) => x.same(y),
        _ => false,
    };
    a.marks == b.marks
        && a.style.same(&b.style)
        && layer(&a.hover, &b.hover)
        && layer(&a.first_letter, &b.first_letter)
        && layer(&a.first_line, &b.first_line)
}

/// Whether two subtrees carry the same values throughout.
pub fn same_tree(a: &Element, b: &Element) -> bool {
    a.node_id == b.node_id && same_tree_as(a, &a.children, b)
}

/// Whether the subtree of `b` carries the values of a root box `shell` (its
/// node id aside) with the children `children` throughout. A host that keeps
/// many subtrees for comparison shares one shell between equal boxes.
///
/// A revised document restyles every box into new style allocations, so equal
/// styles compare field by field. The first walk compares the structure, the
/// texts and the few-word values and stops at a changed text or attribute;
/// only a subtree equal in all of them is walked again for its styles.
pub fn same_tree_as(shell: &Element, children: &Children, b: &Element) -> bool {
    walk(shell, children, b, same_shape) && walk(shell, children, b, same_styles)
}

/// Every pair of the two subtrees passes `same` and has the same node id
/// (the roots aside), and their texts are equal.
fn walk(
    shell: &Element,
    children: &Children,
    b: &Element,
    same: fn(&Element, &Element) -> bool,
) -> bool {
    let mut pending: Vec<(&Element, &Element)> = Vec::new();
    if !same(shell, b) || !same_children(children, &b.children, &mut pending) {
        return false;
    }
    while let Some((x, y)) = pending.pop() {
        if x.node_id != y.node_id
            || !same(x, y)
            || !same_children(&x.children, &y.children, &mut pending)
        {
            return false;
        }
    }
    true
}

/// Compare the child lists `a` and `b` up to their element children, which
/// go to `pending`.
fn same_children<'a>(
    a: &'a Children,
    b: &'a Children,
    pending: &mut Vec<(&'a Element, &'a Element)>,
) -> bool {
    if a.identity() == b.identity() {
        return true;
    }
    if a.len() != b.len() {
        return false;
    }
    for pair in a.iter().zip(b.iter()) {
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
    fn a_shared_shell_compares_like_its_box() {
        let a = block(vec![text("one")]);
        let mut shell = a.clone();
        shell.node_id = 0;
        assert!(same_tree_as(&shell, &a.children, &a));
        assert!(!same_tree_as(&shell, &block(vec![text("onf")]).children, &a));
        let mut other = a.clone();
        other.tag = "p".into();
        assert!(!same_tree_as(&other, &a.children, &a));
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
