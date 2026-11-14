//! What a block record compares with the block of a later build: the root
//! box's values and its children (`dom::same_tree_as`).
//!
//! A record used to keep a copy of the block's element: its tag, attribute
//! strings and marks per block, thousands of copies on a long page of
//! paragraphs of a few classes. Boxes of one kind share one shell instead (an
//! element without children and node id, `dom::same_shell`); the style stages
//! are the box's own (a computed style knows its node), so a record holds its
//! node id and pointers into data the frame tree shares anyway: the shell,
//! the style stages and the child list snapshot.

use crate::dom::{BoxStyles, Children, Element};
use std::rc::Rc;

/// The comparison input of one block record.
pub(crate) struct Source {
    id: u64,
    /// `None` once released (`release`): the record was compared at a
    /// rebuild of the document (`Store::rebase`) and has not been built
    /// since. A `Cell`, not a `RefCell`: the record is shared and its size is
    /// that of a source that cannot be released (the `Rc` niche).
    tree: std::cell::Cell<Option<Tree>>,
}

/// The recorded values of the block.
struct Tree {
    shell: Rc<Element>,
    styles: BoxStyles,
    children: Children,
}

impl Source {
    pub(super) fn id(&self) -> u64 {
        self.id
    }

    /// `f` of the recorded values, if they are held.
    fn with<R>(&self, f: impl FnOnce(&Tree) -> R) -> Option<R> {
        let tree = self.tree.take();
        let out = tree.as_ref().map(f);
        self.tree.set(tree);
        out
    }

    /// The recorded values are held (not `release`d).
    pub(super) fn holds(&self) -> bool {
        self.with(|_| ()).is_some()
    }

    /// Drop the recorded values: the block of a previous frame tree goes
    /// with its document.
    pub(super) fn release(&self) {
        self.tree.take();
    }

    /// The recorded child list.
    pub(super) fn children(&self) -> Option<Children> {
        self.with(|t| t.children.clone())
    }

    /// The subtree of `e` carries the recorded values throughout (a released
    /// source carries none).
    pub(super) fn same(&self, e: &Element) -> bool {
        self.with(|t| {
            self.id == e.node_id && crate::dom::same_tree_as(&t.shell, &t.styles, &t.children, e)
        })
        .unwrap_or(false)
    }

    /// The subtree of `e` lays out like the recorded one: equal values but
    /// for paint-only style values (`dom::same_layout_tree_as`).
    pub(super) fn same_layout(&self, e: &Element) -> bool {
        self.with(|t| {
            self.id == e.node_id
                && crate::dom::same_layout_tree_as(&t.shell, &t.styles, &t.children, e)
        })
        .unwrap_or(false)
    }

    /// The subtree of `e` paints like the recorded one: equal values but for
    /// `color`, which no box of it paints with (`dom::same_unread_color_tree_as`).
    /// The source then holds the values of `e`, so the previous subtree goes
    /// with its document.
    pub(super) fn same_unread_color(&self, e: &Element) -> bool {
        let Some(tree) = self.tree.take() else {
            return false;
        };
        let same = self.id == e.node_id
            && crate::dom::same_unread_color_tree_as(&tree.shell, &tree.styles, &tree.children, e);
        self.tree.set(Some(match same {
            true => Tree {
                shell: tree.shell,
                styles: BoxStyles::of(e),
                children: e.children.clone(),
            },
            false => tree,
        }));
        same
    }

    /// The recorded element, for tests.
    #[cfg(test)]
    pub(super) fn element(&self) -> Option<Element> {
        self.with(|t| Element {
            node_id: self.id,
            children: t.children.clone(),
            ..t.styles.apply(&t.shell)
        })
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
        let shell = match recent.iter().find(|s| crate::dom::same_shell(s, e)) {
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
            tree: std::cell::Cell::new(Some(Tree {
                shell,
                styles: BoxStyles::of(e),
                children: e.children.clone(),
            })),
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

    fn shell(s: &Source) -> Rc<Element> {
        s.with(|t| t.shell.clone()).unwrap()
    }

    #[test]
    fn equal_boxes_share_one_shell_and_each_source_matches_only_its_block() {
        let shells = Shells::default();
        let (a, b, c) = (
            para(1, "t1", "one"),
            para(2, "t1", "two"),
            para(3, "t2", "one"),
        );
        let (sa, sb, sc) = (shells.source(&a), shells.source(&b), shells.source(&c));
        assert!(
            Rc::ptr_eq(&shell(&sa), &shell(&sb)),
            "same class, same shell"
        );
        assert!(!Rc::ptr_eq(&shell(&sa), &shell(&sc)), "another class");
        assert!(sa.same(&a) && sb.same(&b) && sc.same(&c));
        assert!(!sa.same(&b), "another node id");
        assert!(!sb.same(&para(2, "t1", "twp")), "changed text");
        assert!(!sc.same(&para(3, "t1", "one")), "changed attribute");
        assert!(crate::dom::same_tree(&sb.element().unwrap(), &b));
        // A style knows its node: boxes of one kind share the shell anyway.
        let mut d = para(4, "t1", "one");
        d.style = Computed {
            decor_box: 4,
            ..Computed::default()
        }
        .into();
        let sd = shells.source(&d);
        assert!(
            Rc::ptr_eq(&shell(&sa), &shell(&sd)),
            "own styles, same shell"
        );
        let restyled = Element {
            style: a.style,
            ..d.clone()
        };
        assert!(sd.same(&d) && !sd.same(&restyled), "changed style");
        assert!(crate::dom::same_tree(&sd.element().unwrap(), &d));
    }
}
