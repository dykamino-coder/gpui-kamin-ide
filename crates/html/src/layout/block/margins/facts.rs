//! Доказательство пустого схлопывания, общее для одной сборки дерева коробок.
//!
//! Нулевые поля без inline-разрывов и флоатов не требуют ни переписывания
//! потомков, ни построения цепочек. Снимок детей удерживается вместе с ответом:
//! адрес не переиспользуется, а COW-изменение получает отдельную запись.

use crate::dom::{Children, Node};
use crate::layout::box_kind::real_inline;
use crate::style::values::value::Len;
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct MarginFacts {
    children: HashMap<usize, (Children, bool)>,
}

enum Work<'a> {
    Enter(&'a Children),
    Leave(&'a Children),
}

impl MarginFacts {
    pub(crate) fn unchanged(&mut self, nodes: &[Node]) -> bool {
        let mut pending = Vec::new();
        for node in nodes {
            if let Node::Element(e) = node {
                pending.push(Work::Enter(&e.children));
            }
        }
        while let Some(work) = pending.pop() {
            match work {
                Work::Enter(children) => {
                    if self.children.contains_key(&children.identity()) {
                        continue;
                    }
                    pending.push(Work::Leave(children));
                    for node in children.iter() {
                        if let Node::Element(e) = node {
                            pending.push(Work::Enter(&e.children));
                        }
                    }
                }
                Work::Leave(children) => {
                    let unchanged = self.list_unchanged(children);
                    self.children
                        .insert(children.identity(), (children.clone(), unchanged));
                }
            }
        }
        self.list_unchanged(nodes)
    }

    fn list_unchanged(&self, nodes: &[Node]) -> bool {
        nodes.iter().all(|node| {
            let Node::Element(e) = node else { return true };
            let style = e.style.used();
            let zero = |v| matches!(v, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
            !real_inline(e)
                && style.float.is_none_or(|f| f == 0)
                && zero(style.margin.top)
                && zero(style.margin.bottom)
                && self.children[&e.children.identity()].1
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cow_margin_change_invalidates_the_subtree_proof() {
        let mut nodes = vec![Node::Text("leaf".into())];
        for _ in 0..2052 {
            nodes = vec![Node::Element(crate::layout::table::anon::anon_element(
                "div", nodes,
            ))];
        }
        let mut facts = MarginFacts::default();
        assert!(facts.unchanged(&nodes));
        let entries = facts.children.len();
        let Node::Element(root) = &mut nodes[0] else {
            unreachable!()
        };
        let Node::Element(child) = &mut root.children[0] else {
            unreachable!()
        };
        child.style.used_mut().margin.top = Some(Len::Px(10.));
        assert!(!facts.unchanged(&nodes));
        assert_eq!(facts.children.len(), entries + 1);
    }
}
