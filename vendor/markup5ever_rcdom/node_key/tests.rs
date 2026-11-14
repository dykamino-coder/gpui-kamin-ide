//! Ключ живёт с узлом, в том числе вне документа и без коробок стиля.

use super::*;
use crate::{NodeData, RcDom};
use markup5ever::interface::tree_builder::{ElementFlags, NodeOrText, TreeSink};
use markup5ever::{local_name, namespace_url, ns, QualName};

fn element(dom: &RcDom, template: bool) -> Handle {
    dom.create_element(
        QualName::new(None, ns!(html), local_name!("div")),
        vec![],
        ElementFlags {
            template,
            ..Default::default()
        },
    )
}

#[test]
fn pod_layout_and_packing_are_the_host_contract() {
    fn pod<T: Copy + Send + Sync>() {}
    pod::<NodeKey>();
    assert_eq!(mem::size_of::<NodeKey>(), 8);
    assert_eq!(mem::align_of::<NodeKey>(), mem::align_of::<u32>());
    let key = NodeKey {
        slot: 27,
        generation: 42,
    };
    assert_eq!(NodeKey::from_packed(key.packed()), key);
    assert_ne!(key.packed(), 0);
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
    assert!(Rc::ptr_eq(
        &dom.document.resolve_key(keys[0]).unwrap(),
        &branch
    ));
    assert_eq!(keys, [branch.key(), leaf.key(), tail.key()]);
    dom.append_before_sibling(&tail, NodeOrText::AppendNode(branch.clone()));
    assert_eq!(keys, [branch.key(), leaf.key(), tail.key()]);
    assert!(Rc::ptr_eq(&dom.document.children.borrow()[0], &branch));
}

#[test]
fn a_script_handle_keeps_a_detached_subtree_and_registry_alive() {
    let dom = RcDom::default();
    let retained = element(&dom, false);
    dom.append(&retained, NodeOrText::AppendText("child".into()));
    dom.append(&dom.document, NodeOrText::AppendNode(retained.clone()));
    let key = retained.key();
    let child = retained.children.borrow()[0].key();
    let registry = Rc::downgrade(&retained.registry());
    dom.remove_from_parent(&retained);
    drop(dom);
    assert_eq!(retained.key(), key);
    assert!(retained.resolve_key(child).is_some());
    drop(retained);
    assert!(
        registry.upgrade().is_none(),
        "weak slots must not own nodes"
    );
}

#[test]
fn only_actual_drop_reuses_a_slot_with_a_new_generation() {
    let dom = RcDom::default();
    let old = element(&dom, false);
    let key = old.key();
    let held = old.clone();
    let weak = Rc::downgrade(&old);
    drop(old);
    assert!(dom.document.resolve_key(key).is_some());
    drop(held);
    assert!(weak.upgrade().is_none());
    assert!(dom.document.resolve_key(key).is_none());
    let new = element(&dom, false);
    assert_eq!(new.key().slot, key.slot);
    assert_eq!(new.key().generation, key.generation + 1);
    assert!(dom.document.resolve_key(key).is_none());
    assert!(dom
        .document
        .resolve_key(NodeKey {
            slot: u32::MAX,
            generation: 1
        })
        .is_none());
    assert!(dom
        .document
        .resolve_key(NodeKey {
            slot: new.key().slot,
            generation: 0
        })
        .is_none());
}

#[test]
fn generation_exhaustion_retires_the_slot_without_aba() {
    let dom = RcDom::default();
    let registry = dom.document.registry();
    let mut last = admission::infallible(registry.allocate(&admission::Unbounded));
    let slot = last.key.slot;
    registry.slots.borrow_mut().values[slot as usize].generation = u32::MAX;
    last.key.generation = u32::MAX;
    let old = last.key;
    drop(last);
    let next = element(&dom, false);
    assert_ne!(next.key().slot, slot);
    assert!(registry.resolve(old).is_none());
}

#[test]
fn every_kind_including_template_fragment_gets_a_key_at_creation() {
    let dom = RcDom::default();
    let template = element(&dom, true);
    let fragment = match &template.data {
        NodeData::Element {
            template_contents, ..
        } => template_contents.borrow().clone().unwrap(),
        _ => unreachable!(),
    };
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
    let keys: std::collections::HashSet<_> = nodes.iter().map(|node| node.key()).collect();
    assert_eq!(keys.len(), nodes.len());
    for node in &nodes {
        assert!(Rc::ptr_eq(
            &dom.document.resolve_key(node.key()).unwrap(),
            node
        ));
    }
}
