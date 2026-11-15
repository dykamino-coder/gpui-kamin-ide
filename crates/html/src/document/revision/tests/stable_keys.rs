//! Вставка исходного DOM не перенумеровывает существующие коробки.

use crate::{document::Document, dom::Node};
use html5ever::{
    QualName, ns,
    tree_builder::{ElementFlags, NodeOrText, TreeSink},
};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::BTreeMap;

fn ids(nodes: &[Node]) -> BTreeMap<String, u64> {
    let mut found = BTreeMap::new();
    let mut pending: Vec<_> = nodes.iter().collect();
    while let Some(node) = pending.pop() {
        if let Node::Element(e) = node {
            if let Some((_, id)) = e.attrs.iter().find(|(name, _)| name == "id") {
                found.insert(id.clone(), e.node_id);
            }
            pending.extend(e.children.iter());
        }
    }
    found
}

fn named(root: &Handle, id: &str) -> Handle {
    let mut pending = vec![root.clone()];
    while let Some(node) = pending.pop() {
        let hit = match &node.data {
            NodeData::Element { attrs, .. } => attrs
                .borrow()
                .iter()
                .any(|a| &*a.name.local == "id" && &*a.value == id),
            _ => false,
        };
        if hit {
            return node;
        }
        pending.extend(node.children.borrow().iter().cloned());
    }
    panic!("missing {id}")
}

#[test]
fn inserting_a_sibling_keeps_every_existing_node_id() {
    let mut doc = Document::builder(
        "<!doctype html><main id=root><p id=a>A</p><section id=b><b id=c>C</b></section><p id=d>D</p></main>", ""
    ).build().unwrap();
    let before = ids(doc.nodes());
    let input = doc.state.values.traversal.take_source().unwrap();
    let sibling = named(&input.dom.document, "b");
    let inserted = input.dom.create_element(
        QualName::new(None, ns!(html), "aside".into()),
        vec![],
        ElementFlags::default(),
    );
    input
        .dom
        .append(&inserted, NodeOrText::AppendText("new".into()));
    input
        .dom
        .append_before_sibling(&sibling, NodeOrText::AppendNode(inserted));
    doc.state.values.traversal.retain(input);
    doc.apply(&[], "p {color:red}").unwrap();
    assert_eq!(
        ids(doc.nodes()),
        before,
        "inserting a sibling renumbered existing nodes"
    );
}

#[test]
fn restyling_the_same_shape_keeps_the_source_nodes_and_box_keys() {
    let mut doc = Document::builder("<main id=root><p id=a>A</p><p id=b>B</p></main>", "")
        .build()
        .unwrap();
    let root = doc.source_dom();
    let retained = named(&root, "b");
    let before = ids(doc.nodes());
    for (id, key) in &before {
        assert_eq!(named(&root, id).key().packed(), *key);
    }
    doc.update(
        "<main id=root><p id=a>A</p><p id=b>B</p></main>",
        "p {color:red}",
    )
    .unwrap();
    assert!(std::rc::Rc::ptr_eq(&root, &doc.source_dom()));
    assert!(std::rc::Rc::ptr_eq(
        &retained,
        &named(&doc.source_dom(), "b")
    ));
    assert_eq!(ids(doc.nodes()), before);
}

#[test]
fn detach_restyle_and_reinsert_keep_the_original_subtree_keys() {
    let mut doc = Document::builder(
        "<main id=root><section id=a><b id=b>B</b></section><p id=c>C</p></main>",
        "",
    )
    .build()
    .unwrap();
    let before = ids(doc.nodes());
    let input = doc.state.values.traversal.take_source().unwrap();
    let retained = named(&input.dom.document, "a");
    let child = named(&retained, "b");
    let tail = named(&input.dom.document, "c");
    input.dom.remove_from_parent(&retained);
    doc.state.values.traversal.retain(input);
    doc.apply(&[], "p {color:red}").unwrap();
    assert!(std::rc::Rc::ptr_eq(
        &retained,
        &doc.source_dom().resolve_key(retained.key()).unwrap()
    ));
    assert!(doc.source_dom().resolve_key(child.key()).is_some());
    let input = doc.state.values.traversal.take_source().unwrap();
    input
        .dom
        .append_before_sibling(&tail, NodeOrText::AppendNode(retained));
    doc.state.values.traversal.retain(input);
    doc.apply(&[], "p {color:blue}").unwrap();
    assert_eq!(ids(doc.nodes()), before);
}

#[test]
fn replacing_a_document_does_not_alias_an_externally_retained_old_node() {
    let mut doc = Document::builder("<p id=old>old</p>", "").build().unwrap();
    let old = named(&doc.source_dom(), "old");
    let key = old.key();
    doc.update("<main><p id=new>new</p></main>", "").unwrap();
    let registry = doc.source_dom().registry();
    assert!(std::rc::Rc::ptr_eq(&old, &registry.resolve(key).unwrap()));
    assert_ne!(named(&doc.source_dom(), "new").key(), key);
    drop(old);
    assert!(registry.resolve(key).is_none());
    let reused = doc.source_dom().new_in(NodeData::Comment {
        contents: "reuse".into(),
    });
    assert_eq!(reused.key().slot, key.slot);
    assert_eq!(reused.key().generation, key.generation + 1);
    assert!(registry.resolve(key).is_none());
}
