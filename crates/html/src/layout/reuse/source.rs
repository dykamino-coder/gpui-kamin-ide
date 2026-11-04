//! What a block record compares with the block of a later build: the root
//! box's values and its children (`dom::same_tree_as`).
//!
//! A record used to keep a copy of the block's element: its tag, attribute
//! strings and marks per block, thousands of copies on a long page of
//! paragraphs of a few classes. Boxes with equal values share one shell
//! instead (an element without children and node id), so a record holds its
//! node id and two pointers into data the frame tree shares anyway: the shell
//! and the child list snapshot.

use crate::dom::{Children, Element};
use std::rc::Rc;

/// The comparison input of one block record.
pub(crate) struct Source {
    id: u64,
    shell: Rc<Element>,
    children: Children,
}

impl Source {
    pub(super) fn id(&self) -> u64 {
        self.id
    }

    /// The subtree of `e` carries the recorded values throughout.
    pub(super) fn same(&self, e: &Element) -> bool {
        self.id == e.node_id && crate::dom::same_tree_as(&self.shell, &self.children, e)
    }

    /// The recorded element, for tests.
    #[cfg(test)]
    pub(super) fn element(&self) -> Element {
        Element {
            node_id: self.id,
            children: self.children.clone(),
            ..(*self.shell).clone()
        }
    }
}

/// Distinct shells most recently made in one build; equal boxes share them.
#[derive(Default)]
pub(super) struct Shells(std::cell::RefCell<Vec<Rc<Element>>>);

/// Shells kept for sharing: blocks of a flow repeat a few kinds of boxes.
const RECENT: usize = 8;

impl Shells {
    /// The comparison input of block `e`.
    pub(super) fn source(&self, e: &Element) -> Source {
        // Shells hold styles: the list changes whole, without a `&mut` on them.
        let mut recent = self.0.take();
        let shell = match recent.iter().find(|s| crate::dom::same_values(s, e)) {
            Some(shell) => shell.clone(),
            None => {
                let shell = Rc::new(Element {
                    node_id: 0,
                    children: Children::default(),
                    ..e.clone()
                });
                if recent.len() == RECENT {
                    recent.remove(0);
                }
                recent.push(shell.clone());
                shell
            }
        };
        self.0.replace(recent);
        Source {
            id: e.node_id,
            shell,
            children: e.children.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{Marks, Node, Text};
    use crate::style::computed::Computed;

    fn para(id: u64, class: &str, text: &str) -> Element {
        let text = Node::Text(Text::from_source(text.into(), None));
        let mut e = Element::synthetic("p", Computed::default(), Marks::default(), vec![text]);
        e.node_id = id;
        e.attrs.push(("class".into(), class.into()));
        e
    }

    #[test]
    fn equal_boxes_share_one_shell_and_each_source_matches_only_its_block() {
        let shells = Shells::default();
        let (a, b, c) = (para(1, "t1", "one"), para(2, "t1", "two"), para(3, "t2", "one"));
        let (sa, sb, sc) = (shells.source(&a), shells.source(&b), shells.source(&c));
        assert!(Rc::ptr_eq(&sa.shell, &sb.shell), "same class, same shell");
        assert!(!Rc::ptr_eq(&sa.shell, &sc.shell), "another class");
        assert!(sa.same(&a) && sb.same(&b) && sc.same(&c));
        assert!(!sa.same(&b), "another node id");
        assert!(!sb.same(&para(2, "t1", "twp")), "changed text");
        assert!(!sc.same(&para(3, "t1", "one")), "changed attribute");
        assert!(crate::dom::same_tree(&sb.element(), &b));
    }
}
