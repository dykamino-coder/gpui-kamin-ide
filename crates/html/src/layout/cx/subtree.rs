//! Факты поддерева: один итеративный обход каждого COW-снимка за сборку.

use crate::dom::{Children, Element, Node};
use std::collections::HashMap;

/// Отрицательный ответ безопасен для любой геометрии и направления пропагации.
/// Положительный оставляет решение существующему семантическому обходу.
#[derive(Default)]
pub(crate) struct SubtreeFacts {
    children: HashMap<usize, (Children, u8)>,
}

const BREAK: u8 = 1;
const FLOAT: u8 = 2;

fn own(e: &Element) -> u8 {
    let style = e.style.used();
    let breaks = style.break_before_force
        || style.break_after_force
        || style.break_before_avoid
        || style.break_after_avoid;
    (u8::from(breaks) * BREAK) | (u8::from(style.float.is_some_and(|f| f != 0)) * FLOAT)
}

impl SubtreeFacts {
    pub(crate) fn breaks_absent(&mut self, root: &Element) -> bool {
        self.flags(root) & BREAK == 0
    }

    pub(crate) fn floats_absent(&mut self, root: &Element) -> bool {
        self.flags(root) & FLOAT == 0
    }

    fn flags(&mut self, root: &Element) -> u8 {
        let mut pending = vec![(&root.children, false)];
        while let Some((children, leave)) = pending.pop() {
            if self.children.contains_key(&children.identity()) {
                continue;
            }
            if leave {
                let flags = children.iter().fold(0, |flags, node| {
                    flags
                        | match node {
                            Node::Element(e) => own(e) | self.children[&e.children.identity()].1,
                            _ => 0,
                        }
                });
                // Удержание снимка исключает повторное использование адреса.
                self.children
                    .insert(children.identity(), (children.clone(), flags));
            } else {
                pending.push((children, true));
                for node in children.iter() {
                    if let Node::Element(e) = node {
                        pending.push((&e.children, false));
                    }
                }
            }
        }
        own(root) | self.children[&root.children.identity()].1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;

    #[test]
    fn break_proof_is_shared_and_cow_style_change_invalidates_it() {
        let mut root = anon_element("div", vec![Node::Text("leaf".into())]);
        for _ in 0..2052 {
            root = anon_element("div", vec![Node::Element(root)]);
        }
        let mut facts = SubtreeFacts::default();
        assert!(facts.breaks_absent(&root));
        assert!(facts.floats_absent(&root));
        let entries = facts.children.len();
        assert!(facts.breaks_absent(&root));
        assert_eq!(facts.children.len(), entries);
        let Node::Element(child) = &mut root.children[0] else {
            unreachable!()
        };
        child.style.used_mut().break_after_avoid = true;
        child.style.used_mut().float = Some(1);
        assert!(!facts.breaks_absent(&root));
        assert!(!facts.floats_absent(&root));
        assert_eq!(facts.children.len(), entries + 1);
    }
}
