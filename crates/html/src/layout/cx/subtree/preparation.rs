//! Подготовка снимка документа: факты post-order и индексы краски одним обходом.

use super::SubtreeFacts;
use crate::dom::{Element, Node};
use crate::layout::order::{Unkeyed, unkeyed_positioned};
use std::rc::Rc;

pub(crate) struct Preparation {
    pub(crate) facts: Rc<SubtreeFacts>,
    pub(crate) unkeyed: Rc<Unkeyed>,
    pub(crate) clearance_possible: bool,
}

type Snapshot = ((usize, usize), Rc<Preparation>);

#[derive(Default)]
pub(crate) struct PreparationCache {
    snapshot: std::cell::Cell<Option<Snapshot>>,
}

impl PreparationCache {
    /// Только Document гарантирует неизменность корневого списка. Для raw
    /// render адрес не доказывает неизменность элементов и кэш не применяется.
    pub(crate) fn get(&self, nodes: &[Node], stable: bool, changed: bool) -> Rc<Preparation> {
        let key = (nodes.as_ptr() as usize, nodes.len());
        let snapshot = self.snapshot.take();
        let prepared = match snapshot {
            Some((old, prepared)) if stable && !changed && old == key => prepared,
            _ => Rc::new(Preparation::new(nodes)),
        };
        if stable {
            self.snapshot.set(Some((key, prepared.clone())));
        }
        prepared
    }
}

impl Preparation {
    fn new(nodes: &[Node]) -> Self {
        crate::work::add!(FullTreeWalks, 1);
        let mut facts = SubtreeFacts::default();
        let mut map = std::collections::HashMap::new();
        let mut at = 0;
        let mut last = None;
        let mut pending: Vec<(&Element, bool)> = nodes
            .iter()
            .rev()
            .filter_map(|node| {
                if let Node::Element(e) = node {
                    Some((e, false))
                } else {
                    None
                }
            })
            .collect();
        while let Some((e, leave)) = pending.pop() {
            if leave {
                let flags = facts.list_flags(&e.children);
                facts
                    .children
                    .insert(e.children.identity(), (e.children.mark(), flags));
                // Конец поддерева исключает собственных потомков из проверки
                // более позднего позиционированного (CSS2 Appendix E, шаг 8).
                map.insert(e.node_id, at);
            } else {
                crate::work::add!(TreeNodesVisited, 1);
                if unkeyed_positioned(e) {
                    last = Some(at);
                }
                at += 1;
                pending.push((e, true));
                for node in e.children.iter().rev() {
                    if let Node::Element(child) = node {
                        pending.push((child, false));
                    }
                }
            }
        }
        let clearance_possible = facts.clearance_possible(nodes);
        Self {
            facts: Rc::new(facts),
            unkeyed: Rc::new((map, last)),
            clearance_possible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;
    use crate::style::computed::Position;

    #[test]
    fn stable_snapshot_reuses_preparation_until_computed_publication_changes() {
        let nodes = vec![Node::Element(anon_element("div", vec![]))];
        let cache = PreparationCache::default();
        let first = cache.get(&nodes, true, true);
        assert!(Rc::ptr_eq(&first, &cache.get(&nodes, true, false)));
        let changed = cache.get(&nodes, true, true);
        assert!(!Rc::ptr_eq(&first, &changed));
        assert!(Rc::ptr_eq(&changed, &cache.get(&nodes, true, false)));
    }

    #[test]
    fn raw_tree_is_recomputed_even_if_its_address_does_not_change() {
        let mut nodes = vec![Node::Element(anon_element("div", vec![]))];
        let cache = PreparationCache::default();
        let first = cache.get(&nodes, false, false);
        assert!(!first.clearance_possible);
        let Node::Element(e) = &mut nodes[0] else {
            unreachable!()
        };
        e.style.used_mut().float = Some(1);
        let next = cache.get(&nodes, false, false);
        assert!(next.clearance_possible);
        assert!(!Rc::ptr_eq(&first, &next));
    }

    #[test]
    fn paint_indices_use_preorder_entries_and_subtree_ends() {
        let mut positioned = anon_element("span", vec![]);
        positioned.node_id = 2;
        positioned.style.used_mut().position = Some(Position::Relative);
        let positioned_id = positioned.node_id;
        let mut root = anon_element("div", vec![Node::Element(positioned)]);
        root.node_id = 1;
        let root_id = root.node_id;
        let mut sibling = anon_element("div", vec![]);
        sibling.node_id = 3;
        let sibling_id = sibling.node_id;
        let nodes = [
            Node::Element(root),
            Node::Text("ignored".into()),
            Node::Element(sibling),
        ];
        let prepared = Preparation::new(&nodes);
        assert_eq!(prepared.unkeyed.1, Some(1));
        assert_eq!(prepared.unkeyed.0[&positioned_id], 2);
        assert_eq!(prepared.unkeyed.0[&root_id], 2);
        assert_eq!(prepared.unkeyed.0[&sibling_id], 3);
    }
}
