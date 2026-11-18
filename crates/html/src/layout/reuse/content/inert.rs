//! Whether a content's subtree may replay (`key::plain`), decided once per
//! children list of a build.
//!
//! A container's content asks this of its whole subtree, and every container
//! on the path to an edit is built, so a walk per container is quadratic in
//! depth (a chain of 900 blocks: about 400 000 visits per frame). LayoutNG and
//! Stylo keep such facts on the node, as the box summary of a children
//! snapshot here does (`Children::box_summary`); this verdict also reads the
//! build's value context (viewport lengths), so it lives with the build
//! instead: one verdict per list (`Children::identity`), the list held so its
//! address is not reused while the verdict stands. Each element of the frame
//! is checked at most once, and the walk is iterative, as deep trees are legal
//! input.

use crate::dom::{Children, Element, Node};
use std::cell::RefCell;
use std::collections::HashMap;

/// The verdicts of the children lists of one build.
#[derive(Default)]
pub(crate) struct InertLists(RefCell<HashMap<usize, (Children, bool)>>);

impl InertLists {
    /// Every element of `nodes` and of their subtrees passes `own`.
    pub(super) fn all(&self, nodes: &[Node], own: impl Fn(&Element) -> bool) -> bool {
        nodes.iter().all(|node| match node {
            Node::Element(e) => own(e) && self.list(&e.children, &own),
            _ => true,
        })
    }

    fn known(&self, list: &Children) -> Option<bool> {
        if list.is_empty() {
            return Some(true);
        }
        self.0.borrow().get(&list.identity()).map(|(_, ok)| *ok)
    }

    fn set(&self, list: &Children, ok: bool) {
        self.0
            .borrow_mut()
            .insert(list.identity(), (list.clone(), ok));
    }

    /// Every element of `root` and below passes `own`; the verdict of every
    /// list the walk finished (and of every open one, on a failure) is kept.
    fn list(&self, root: &Children, own: &impl Fn(&Element) -> bool) -> bool {
        if let Some(ok) = self.known(root) {
            return ok;
        }
        let mut open: Vec<(&Children, std::slice::Iter<'_, Node>)> = vec![(root, root.iter())];
        loop {
            let Some(next) = open.last_mut().map(|(_, level)| level.next()) else {
                return true;
            };
            match next {
                Some(Node::Element(e)) => {
                    let ok = own(e)
                        && match self.known(&e.children) {
                            Some(ok) => ok,
                            None => {
                                open.push((&e.children, e.children.iter()));
                                continue;
                            }
                        };
                    if !ok {
                        for (list, _) in &open {
                            self.set(list, false);
                        }
                        return false;
                    }
                }
                Some(_) => {}
                None => {
                    if let Some((list, _)) = open.pop() {
                        self.set(list, true);
                    }
                    if open.is_empty() {
                        return true;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
