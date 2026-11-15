//! Контракты публикации коробок: стабильные ключи, связи и ревизии документа.

use super::*;
use crate::document::Document;
use std::rc::Rc;

fn element(id: u64, children: Vec<Node>) -> Node {
    let mut e = crate::layout::table::anon::anon_element("div", children);
    e.node_id = id;
    Node::Element(e)
}

fn ids(tree: &BoxTree) -> Vec<BoxId> {
    let mut out = Vec::new();
    let mut pending = vec![tree.first];
    while let Some(id) = pending.pop() {
        let Some(record) = tree.get(id) else { continue };
        out.push(id);
        pending.push(record.next);
        pending.push(record.first);
    }
    out
}

#[test]
fn publication_changes_links_in_place_and_invalidates_removed_slots() {
    let before = vec![element(1, vec![element(2, vec![]), element(3, vec![])])];
    let mut tree = BoxTree::default();
    tree.publish(&before);
    let old = ids(&tree);
    assert!(old.iter().all(|id| id.is_some()));
    assert_eq!(tree.get(old[0]).unwrap().first, old[1]);
    assert_eq!(tree.get(old[1]).unwrap().next, old[2]);
    let after = vec![element(1, vec![element(3, vec![])])];
    tree.publish(&after);
    assert_eq!(ids(&tree), [old[0], old[2]]);
    assert!(tree.get(old[1]).is_none());
    assert_eq!(tree.get(old[0]).unwrap().first, old[2]);
    assert_eq!(tree.get(old[2]).unwrap().parent, old[0]);
    assert_eq!(tree.get(old[2]).unwrap().next, BoxId::NONE);
}

#[test]
fn commit_and_rebuild_keep_the_same_arena_and_box_keys() {
    let html =
        "<!doctype html><html style='width:800px'><body><div><p>a</p><p>b</p></div></body></html>";
    let theme = ".on { color:red }";
    let mut doc = Document::builder(html, theme).build().unwrap();
    let arena = doc.state.boxes.clone();
    let initial = ids(&arena.borrow());
    let epoch = arena.borrow().epoch;
    let clean = html.replace("<p>a", "<p class=unmentioned>a");
    doc.update(&clean, theme).unwrap();
    assert!(Rc::ptr_eq(&arena, &doc.state.boxes));
    assert_eq!(ids(&arena.borrow()), initial);
    assert_eq!(
        arena.borrow().epoch,
        epoch,
        "clean commit must not republish"
    );
    let restyled = clean.replace("class=unmentioned", "class=on");
    doc.update(&restyled, theme).unwrap();
    assert!(Rc::ptr_eq(&arena, &doc.state.boxes));
    assert_eq!(ids(&arena.borrow()), initial);
    let text = restyled.replace(">b</p>", ">different</p>");
    doc.update(&text, theme).unwrap();
    assert!(Rc::ptr_eq(&arena, &doc.state.boxes));
    assert_eq!(ids(&arena.borrow()), initial);
    assert!(initial.iter().all(|&id| arena.borrow().get(id).is_some()));
}

#[test]
fn a_new_shape_does_not_resolve_old_parse_local_source_ids() {
    let mut doc = Document::builder("<p>a</p>", "").build().unwrap();
    let old = ids(&doc.state.boxes.borrow());
    doc.update("<div><p>a</p></div>", "").unwrap();
    let arena = doc.state.boxes.borrow();
    assert!(old.iter().all(|&id| arena.get(id).is_none()));
    assert!(ids(&arena).iter().all(|&id| arena.get(id).is_some()));
}

fn sources(nodes: &[Node]) -> Vec<u64> {
    let mut pending: Vec<_> = nodes.iter().rev().collect();
    let mut out = Vec::new();
    while let Some(node) = pending.pop() {
        if let Node::Element(e) = node {
            out.push(e.node_id);
            pending.extend(e.children.iter().rev());
        }
    }
    out
}

#[test]
fn full_restyles_publish_the_accepted_box_topology() {
    let before = "<!doctype html><main><p>a</p><p>b</p></main>";
    let after = before.replace("<p>a", "<p class=hidden>a");
    let theme = ".hidden { display:none }";
    for records in [false, true] {
        let mut doc = Document::builder(before, theme).build().unwrap();
        let arena = doc.state.boxes.clone();
        let old = ids(&arena.borrow());
        let old_origins: Vec<_> = old
            .iter()
            .map(|&id| arena.borrow().get(id).unwrap().origin.clone())
            .collect();
        let old_sources = sources(&doc.nodes);
        if records {
            let mutations = doc.mutations(&after).unwrap();
            doc.apply(&mutations, theme).unwrap();
        } else {
            doc.update(&after, theme).unwrap();
        }
        assert!(Rc::ptr_eq(&arena, &doc.state.boxes));
        let tree = arena.borrow();
        let actual: Vec<_> = ids(&tree)
            .iter()
            .map(|&id| tree.get(id).unwrap().source)
            .collect();
        let accepted = sources(&doc.nodes);
        let accepted_origins: Vec<_> = ids(&tree)
            .iter()
            .map(|&id| tree.get(id).unwrap().origin.clone())
            .collect();
        assert_ne!(accepted, old_sources, "the restyle changes the box tree");
        assert_eq!(actual, accepted, "records={records}");
        for (id, origin) in old.into_iter().zip(old_origins) {
            if !accepted_origins.contains(&origin) {
                assert!(tree.get(id).is_none(), "removed box {origin:?}");
            }
        }
    }
}

#[test]
fn update_hiding_a_sibling_does_not_transfer_its_box_key() {
    hiding_a_sibling(false);
}

#[test]
fn apply_hiding_a_sibling_does_not_transfer_its_box_key() {
    hiding_a_sibling(true);
}

fn hiding_a_sibling(records: bool) {
    let before = "<!doctype html><main><p id=left>a</p><p id=right>b</p></main>";
    let after = before.replace("id=left", "id=left class=hidden");
    let theme = ".hidden { display:none }";
    let source = |nodes: &[Node], name: &str| {
        let mut pending: Vec<_> = nodes.iter().collect();
        while let Some(node) = pending.pop() {
            if let Node::Element(e) = node {
                if e.attrs
                    .iter()
                    .any(|(key, value)| key == "id" && value == name)
                {
                    return e.node_id;
                }
                pending.extend(e.children.iter());
            }
        }
        panic!("missing element {name}");
    };
    let key = |tree: &BoxTree, source| {
        ids(tree)
            .into_iter()
            .find(|&id| tree.get(id).unwrap().source == source)
            .expect("authored box")
    };
    let mut doc = Document::builder(before, theme).build().unwrap();
    let root = doc.source_dom();
    let left = key(&doc.state.boxes.borrow(), source(&doc.nodes, "left"));
    let right = key(&doc.state.boxes.borrow(), source(&doc.nodes, "right"));
    let right_flow = doc.state.boxes.borrow().get(right).unwrap().inline.clone();
    let right_key = doc.state.values.traversal.element_keys()[&source(&doc.nodes, "right")];
    if records {
        doc.apply(&doc.mutations(&after).unwrap(), theme).unwrap();
    } else {
        doc.update(&after, theme).unwrap();
    }
    assert!(Rc::ptr_eq(&root, &doc.source_dom()), "same live DOM");
    let tree = doc.state.boxes.borrow();
    assert_eq!(
        key(&tree, source(&doc.nodes, "right")),
        right,
        "unchanged sibling must retain its own key, records={records}"
    );
    assert_eq!(tree.get(right).unwrap().origin, Source::Node(right_key));
    assert!(Rc::ptr_eq(
        right_flow.as_ref().unwrap(),
        tree.get(right).unwrap().inline.as_ref().unwrap()
    ));
    assert!(tree.get(left).is_none(), "hidden box must be retired");
    drop(tree);
    if records {
        doc.apply(&doc.mutations(before).unwrap(), theme).unwrap();
    } else {
        doc.update(before, theme).unwrap();
    }
    let tree = doc.state.boxes.borrow();
    assert_eq!(key(&tree, source(&doc.nodes, "right")), right);
    assert_ne!(key(&tree, source(&doc.nodes, "left")), left);
    assert!(tree.get(left).is_none(), "old hidden key stays stale");
}

#[test]
fn rejected_cascade_keeps_box_records_and_keys_unchanged() {
    use crate::budget::{BudgetKind, DocumentLimits, ResourceGovernor};
    let before = "<!doctype html><main><p>a</p><p>b</p></main>";
    let after = before.replace("<p>a", "<p class=hidden>a");
    let theme = ".hidden{display:none}p{--a:green;color:var(--a)}";
    let snapshot = |tree: &BoxTree| {
        ids(tree)
            .into_iter()
            .map(|id| {
                let r = tree.get(id).unwrap();
                (id, r.parent, r.first, r.next, r.source, r.epoch)
            })
            .collect::<Vec<_>>()
    };
    for records in [false, true] {
        let mut doc = Document::builder(before, "")
            .limits(
                DocumentLimits::default()
                    .lower(BudgetKind::SubstitutionBytes, 1)
                    .unwrap(),
            )
            .governor(&ResourceGovernor::default())
            .build()
            .unwrap();
        let arena = doc.state.boxes.clone();
        let old = snapshot(&arena.borrow());
        if records {
            assert!(doc.apply(&doc.mutations(&after).unwrap(), theme).is_err());
        } else {
            assert!(doc.update(&after, theme).is_err());
        }
        assert!(Rc::ptr_eq(&arena, &doc.state.boxes));
        assert_eq!(snapshot(&arena.borrow()), old, "records={records}");
        assert_eq!(sources(&doc.nodes).len(), old.len());
    }
}
