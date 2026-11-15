//! Ключ живёт с узлом, в том числе вне документа и без коробок стиля.

use super::*;
use crate::{NodeData, RcDom};
use markup5ever::interface::tree_builder::{ElementFlags, NodeOrText, TreeSink};
use markup5ever::{local_name, ns, QualName};
use std::{collections::HashSet, mem, rc::Rc};

fn element(dom: &RcDom, template: bool) -> Handle {
    let mut flags = ElementFlags::default();
    flags.template = template;
    dom.create_element(
        QualName::new(None, ns!(html), local_name!("div")),
        vec![],
        flags,
    )
}

fn template_contents(node: &Handle) -> Handle {
    match &node.data {
        NodeData::Element {
            template_contents, ..
        } => template_contents.borrow().clone().unwrap(),
        _ => unreachable!(),
    }
}

#[test]
fn pod_layout_and_packing_are_the_host_contract() {
    fn pod<T: Copy + Send + Sync>() {}
    pod::<NodeKey>();
    assert_eq!(mem::size_of::<NodeKey>(), 8);
    let key = NodeKey::from_packed(27 << 32 | 42);
    assert_eq!(NodeKey::from_packed(key.packed()), key);
    assert_eq!(NodeKey::NONE.packed(), 0);
}

#[test]
fn a_node_spends_one_word_on_its_key() {
    // The key is the node's only identity storage: no registry pointer and
    // no slot table beside it.
    assert_eq!(mem::size_of::<Cell<NodeKey>>(), mem::size_of::<u64>());
}

#[test]
fn remove_and_reinsert_preserve_every_subtree_key_and_following_sibling() {
    let dom = RcDom::default();
    let branch = element(&dom, false);
    let leaf = element(&dom, false);
    let tail = element(&dom, false);
    dom.append(&branch, NodeOrText::AppendNode(leaf.clone()));
    dom.append(&dom.document, NodeOrText::AppendNode(branch.clone()));
    dom.append(&dom.document, NodeOrText::AppendNode(tail.clone()));
    let keys = [branch.key(), leaf.key(), tail.key()];
    dom.remove_from_parent(&branch);
    assert_eq!(keys, [branch.key(), leaf.key(), tail.key()]);
    dom.append_before_sibling(&tail, NodeOrText::AppendNode(branch.clone()));
    assert_eq!(keys, [branch.key(), leaf.key(), tail.key()]);
    assert!(Rc::ptr_eq(&dom.document.children.borrow()[0], &branch));
}

#[test]
fn a_detached_subtree_keeps_its_keys_after_its_document_is_gone() {
    let dom = RcDom::default();
    let retained = element(&dom, false);
    dom.append(&retained, NodeOrText::AppendText("child".into()));
    dom.append(&dom.document, NodeOrText::AppendNode(retained.clone()));
    let key = retained.key();
    let child = retained.children.borrow()[0].key();
    dom.remove_from_parent(&retained);
    drop(dom);
    assert_eq!(retained.key(), key);
    assert_eq!(retained.children.borrow()[0].key(), child);
}

#[test]
fn keys_depend_only_on_the_document_input_not_on_other_documents() {
    let build = || {
        let dom = RcDom::default();
        let a = element(&dom, false);
        let b = element(&dom, true);
        dom.append(&a, NodeOrText::AppendText("t".into()));
        let keys = [a.key(), b.key(), a.children.borrow()[0].key()];
        (dom, keys)
    };
    let (_first, first) = build();
    let _other: Vec<_> = (0..10).map(|_| RcDom::default()).collect();
    let (_second, second) = build();
    assert_eq!(first, second);
}

#[test]
fn a_dropped_node_key_is_never_issued_again() {
    let dom = RcDom::default();
    let old = element(&dom, false);
    let key = old.key();
    drop(old);
    let fresh: HashSet<_> = (0..100).map(|_| element(&dom, false).key()).collect();
    assert!(!fresh.contains(&key));
    assert_eq!(fresh.len(), 100);
}

#[test]
fn a_reparse_continues_the_numbering_of_the_previous_document() {
    let old = RcDom::default();
    let retained = element(&old, false);
    let next = RcDom::try_new_from(&admission::Unbounded, old.next_key()).unwrap();
    let new = element(&next, false);
    assert_ne!(new.key(), retained.key());
    assert_ne!(next.document.key(), retained.key());
    assert_ne!(next.document.key(), old.document.key());
}

#[test]
fn adopt_rekeys_a_foreign_or_unkeyed_subtree_as_a_mutation() {
    let dom = RcDom::default();
    let own: Vec<_> = (0..3).map(|_| element(&dom, false)).collect();
    // A node created outside any document has no key until adopted.
    let loose = Node::new(NodeData::Comment {
        contents: "x".into(),
    });
    assert_eq!(loose.key(), NodeKey::NONE);
    // A subtree from another document carries that document's numbers,
    // which collide with this document's.
    let other = RcDom::default();
    let foreign = element(&other, true);
    other.append(&foreign, NodeOrText::AppendText("t".into()));
    let contents = template_contents(&foreign);
    other.append(&contents, NodeOrText::AppendText("in".into()));
    foreign.children.borrow_mut().push(loose.clone());
    let subtree = [
        foreign.clone(),
        foreign.children.borrow()[0].clone(),
        loose.clone(),
        contents.clone(),
        contents.children.borrow()[0].clone(),
    ];
    assert!(subtree
        .iter()
        .any(|n| own.iter().any(|o| o.key() == n.key())));
    dom.adopt(&foreign);
    let mut keys: HashSet<_> = own.iter().map(|n| n.key()).collect();
    keys.insert(dom.document.key());
    for node in &subtree {
        assert_ne!(node.key(), NodeKey::NONE);
        assert!(
            keys.insert(node.key()),
            "adopt reused a key of this document"
        );
    }
    assert!(!keys.contains(&NodeKey::from_packed(dom.next_key())));
}

#[test]
fn every_kind_including_template_fragment_gets_a_key_at_creation() {
    let dom = RcDom::default();
    let template = element(&dom, true);
    let fragment = template_contents(&template);
    let comment = dom.create_comment("x".into());
    let pi = dom.create_pi("target".into(), "value".into());
    dom.append(&fragment, NodeOrText::AppendText("text".into()));
    dom.append_doctype_to_document("html".into(), "".into(), "".into());
    let nodes = [
        dom.document.clone(),
        template,
        fragment.clone(),
        comment,
        pi,
        fragment.children.borrow()[0].clone(),
        dom.document.children.borrow()[0].clone(),
    ];
    let keys: HashSet<_> = nodes.iter().map(|node| node.key()).collect();
    assert_eq!(keys.len(), nodes.len());
    assert!(!keys.contains(&NodeKey::NONE));
}
