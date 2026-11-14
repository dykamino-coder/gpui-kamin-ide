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
    let clean = html.replace("<p>a", "<p class=unmentioned>a");
    doc.update(&clean, theme).unwrap();
    assert!(Rc::ptr_eq(&arena, &doc.state.boxes));
    assert_eq!(ids(&arena.borrow()), initial);
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
