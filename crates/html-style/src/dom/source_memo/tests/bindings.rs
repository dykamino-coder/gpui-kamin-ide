//! Memo entries capture exactly the binding capability published by the full walk.
use super::*;
use crate::dom::TextId;
use crate::values::bindings::BindingStore;

fn text(parent: &Handle) -> Handle {
    parent
        .children
        .borrow()
        .iter()
        .find(|node| matches!(node.data, NodeData::Text { .. }))
        .unwrap()
        .clone()
}

#[test]
fn captured_parent_segment_has_only_direct_identity_and_text_bindings() {
    let (input, values, cache, _) =
        build("<!doctype html><main id=p>parent<span id=c>child</span></main>");
    let p = handle(&input, "p");
    let c = handle(&input, "c");
    let pt = text(&p);
    let ct = text(&c);
    let parent = cache.raw().get(p.key()).unwrap();
    let child = cache.raw().get(c.key()).unwrap();
    let current = values.traversal.binding_store();
    assert!(Rc::ptr_eq(
        &current.borrow().segment(p.key()).unwrap(),
        &parent.bindings
    ));
    assert!(Rc::ptr_eq(
        &current.borrow().segment(c.key()).unwrap(),
        &child.bindings
    ));
    assert_eq!(parent.bindings.owner(), p.key());
    let mut isolated = BindingStore::default();
    let mut stage = isolated.stage();
    stage.replace(parent.bindings.clone()).unwrap();
    let update = stage.into_update();
    let admission = Admission::new(input.context().budget().clone());
    isolated
        .prepare(update, std::iter::empty(), &admission)
        .unwrap()
        .unwrap()
        .publish();
    assert_eq!(isolated.id(&p), Some(p.key().packed()));
    assert_eq!(isolated.id(&c), None);
    assert!(
        isolated
            .text(TextId::new(pt.key().packed()).unwrap())
            .is_some()
    );
    assert!(
        isolated
            .text(TextId::new(ct.key().packed()).unwrap())
            .is_none()
    );
}

#[test]
fn full_walk_publishes_an_explicit_empty_root_segment() {
    let (_, values, _, _) = build("<!doctype html><p>plain</p>");
    let store = values.traversal.binding_store();
    assert_eq!(
        store.borrow().segment(NodeKey::NONE).unwrap().owner(),
        NodeKey::NONE
    );
}

#[test]
fn held_raw_entries_and_capabilities_do_not_keep_document_values_alive() {
    let (input, values, cache, _) = build(
        "<!doctype html><main id=p style='width:calc(10px + 20%)'>parent<span>child</span></main>",
    );
    let entry = cache.raw().get(handle(&input, "p").key()).unwrap().clone();
    let weak = Rc::downgrade(&values);
    drop(values);
    assert!(weak.upgrade().is_none());
    assert_eq!(entry.bindings.owner(), entry.node);
    assert!(!entry.raw.is_empty());
}
