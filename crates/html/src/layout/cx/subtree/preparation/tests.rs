//! Подготовка снимка: полный обход, продолжение от прошлой ревизии и индексы краски.

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
    e.style.used_mut().reset_box_layout.float = Some(1);
    let next = cache.get(&nodes, false, false);
    assert!(next.clearance_possible);
    assert!(!Rc::ptr_eq(&first, &next));
}

fn el(id: u64, children: Vec<Node>) -> Node {
    let mut e = anon_element("div", children);
    e.node_id = id;
    Node::Element(e)
}

fn lists<'a>(nodes: &'a [Node], out: &mut Vec<&'a crate::dom::Children>) {
    for node in nodes {
        if let Node::Element(e) = node {
            out.push(&e.children);
            lists(&e.children, out);
        }
    }
}

#[test]
fn continued_preparation_visits_only_the_copied_lists_and_equals_a_full_one() {
    let mut positioned = anon_element("span", vec![Node::Text("p".into())]);
    positioned.node_id = 3;
    positioned.style.used_mut().reset_positioned.position = Some(Position::Relative);
    let nodes = vec![el(
        1,
        vec![
            el(2, vec![Node::Text("a".into())]),
            Node::Element(positioned),
            el(4, vec![el(5, vec![Node::Text("x".into())])]),
        ],
    )];
    let cache = PreparationCache::default();
    cache.get(&nodes, true, true);
    // Правка текста копирует только списки на пути: корня, 4 и 5.
    let mut next = nodes.clone();
    let Node::Element(root) = &mut next[0] else {
        unreachable!()
    };
    let Node::Element(four) = &mut root.children[2] else {
        unreachable!()
    };
    let Node::Element(five) = &mut four.children[0] else {
        unreachable!()
    };
    five.children[0] = Node::Text("y".into());
    // Прошлое дерево живо и не тронуто: копия при записи отделила пути.
    assert_ne!(format!("{nodes:?}"), format!("{next:?}"));
    let mut all = Vec::new();
    lists(&next, &mut all);
    assert_eq!(
        all.iter()
            .filter(|list| list.box_summary().is_none())
            .count(),
        3
    );
    let continued = PreparationCache::continuing(&cache).get(&next, true, false);
    let summaries: Vec<_> = all.iter().map(|list| list.box_summary()).collect();
    assert!(summaries.iter().all(Option::is_some));
    let full = Preparation::new(&next);
    assert_eq!(continued.unkeyed.0, full.unkeyed.0);
    assert_eq!(continued.unkeyed.1, full.unkeyed.1);
    assert_eq!(continued.clearance_possible, full.clearance_possible);
    assert_eq!(
        summaries,
        all.iter()
            .map(|list| list.box_summary())
            .collect::<Vec<_>>()
    );
    // Пересчёт стиля (`changed`) готовит дерево полным обходом.
    let again = PreparationCache::continuing(&cache).get(&next, true, true);
    assert!(!Rc::ptr_eq(&continued.unkeyed, &again.unkeyed));
}

#[test]
fn paint_indices_use_preorder_entries_and_subtree_ends() {
    let mut positioned = anon_element("span", vec![]);
    positioned.node_id = 2;
    positioned.style.used_mut().reset_positioned.position = Some(Position::Relative);
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

#[cfg(feature = "work-counters")]
#[test]
fn prepared_cells_read_retained_facts_without_subtree_queries() {
    use crate::work::{Counter, Session};
    let nodes = vec![el(
        1,
        (2..1026)
            .map(|id| el(id, vec![el(id + 1024, vec![])]))
            .collect(),
    )];
    Preparation::new(&nodes);
    let Node::Element(table) = &nodes[0] else {
        unreachable!()
    };
    let session = Session::begin();
    for node in table.children.iter() {
        let Node::Element(cell) = node else {
            unreachable!()
        };
        assert!(
            !SubtreeFacts::default()
                .prepared_list(&cell.children)
                .expect("prepared cell")
                .margins
        );
    }
    let work = session.finish();
    let total = |counter: Counter| {
        work.0
            .iter()
            .map(|phase| phase[counter as usize])
            .sum::<u64>()
    };
    assert_eq!(total(Counter::SubtreeQueries), 0);
    assert_eq!(total(Counter::SubtreeNodesVisited), 1024);
}
