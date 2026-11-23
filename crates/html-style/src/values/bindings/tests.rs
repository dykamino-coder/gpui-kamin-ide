//! Binding ownership, admission, publication, and cancellation contracts.
use super::*;
use crate::budget::admission::Admission;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

fn text(dom: &RcDom, content: &str) -> Handle {
    dom.new_node(NodeData::Text {
        contents: std::cell::RefCell::new(content.into()),
    })
}

fn segment(owner: NodeKey, node: &Handle, read: bool) -> Rc<BindingSegment> {
    let mut recorder = BindingRecorder::default();
    recorder.begin_owner(owner);
    recorder.text(node);
    if read {
        recorder.read(node);
    }
    recorder.end_owner(&Admission::default()).unwrap()
}

fn install(store: &mut BindingStore, segments: &[Rc<BindingSegment>], retired: &[NodeKey]) {
    let mut stage = store.stage();
    for segment in segments {
        stage.replace(segment.clone()).unwrap();
    }
    let update = stage.into_update();
    store
        .prepare(update, retired.iter().copied(), &Admission::default())
        .unwrap()
        .unwrap()
        .publish();
}

#[test]
fn source_identity_survives_insert_and_move_without_owning_dom() {
    let dom = RcDom::default();
    let original = text(&dom, "old");
    let id = TextId::new(original.key().packed()).unwrap();
    let weak = Rc::downgrade(&original);
    let owner = NodeKey::from_packed(10);
    let old = segment(owner, &original, false);
    let mut store = BindingStore::default();
    install(&mut store, std::slice::from_ref(&old), &[]);
    let inserted = text(&dom, "new earlier sibling");
    let moved = segment(NodeKey::from_packed(11), &original, false);
    install(
        &mut store,
        &[moved, segment(NodeKey::from_packed(12), &inserted, false)],
        &[owner],
    );
    assert_eq!(store.text(id), Some(Rc::as_ptr(&original) as usize));
    assert!(!store.stage().resume(&old));
    drop(original);
    assert!(weak.upgrade().is_none());
}

#[test]
fn resume_is_a_current_capability_and_abort_is_atomic() {
    let dom = RcDom::default();
    let node = text(&dom, "text");
    let owner = NodeKey::from_packed(10);
    let old = segment(owner, &node, true);
    let mut store = BindingStore::default();
    install(&mut store, std::slice::from_ref(&old), &[]);
    let alien = segment(owner, &node, true);
    let mut stage = store.stage();
    assert!(stage.resume(&old));
    assert!(!stage.resume(&alien));
    assert!(stage.resume(&old)); // a rejected capability did not write anything
    stage.replace(alien.clone()).unwrap();
    assert!(!stage.resume(&old));
    drop(stage);
    assert!(store.stage().resume(&old));
    let mut stage = store.stage();
    stage.replace(alien).unwrap();
    let update = stage.into_update();
    let publication = store
        .prepare(update, [], &Admission::default())
        .unwrap()
        .unwrap();
    drop(publication);
    assert!(store.stage().resume(&old));
}

#[test]
fn retiring_one_reader_preserves_the_other_and_removes_last_read() {
    let dom = RcDom::default();
    let node = text(&dom, "shared read");
    let (a, b) = (NodeKey::from_packed(10), NodeKey::from_packed(11));
    let mut store = BindingStore::default();
    install(
        &mut store,
        &[segment(a, &node, true), segment(b, &node, true)],
        &[],
    );
    install(&mut store, &[], &[a]);
    assert!(store.was_read(node.key(), Rc::as_ptr(&node) as usize));
    install(&mut store, &[], &[b]);
    assert!(!store.was_read(node.key(), Rc::as_ptr(&node) as usize));
    assert!(store.nodes.is_empty());
}

#[test]
fn ten_thousand_replacements_abort_and_held_segments_do_not_grow_current_map() {
    let dom = RcDom::default();
    let owner = NodeKey::from_packed(10);
    let first = text(&dom, "held old frame");
    let held = segment(owner, &first, true);
    let mut store = BindingStore::default();
    install(&mut store, std::slice::from_ref(&held), &[]);
    for n in 0..10_000 {
        let node = text(&dom, &n.to_string());
        let replacement = segment(owner, &node, true);
        let weak = Rc::downgrade(&replacement);
        {
            let mut stage = store.stage();
            stage.replace(replacement.clone()).unwrap();
        }
        install(&mut store, &[replacement], &[]);
        assert_eq!(store.nodes.len(), 1);
        assert_eq!(store.segments.len(), 1);
        assert_eq!(weak.strong_count(), 1);
        assert!(!store.stage().resume(&held));
    }
    install(&mut store, &[], &[owner]);
    assert!(store.nodes.is_empty());
    assert!(store.segments.is_empty());
    assert_eq!(held.direct.len(), 1);
}

#[test]
fn stale_and_conflicting_publications_leave_committed_bindings_unchanged() {
    let dom = RcDom::default();
    let node = text(&dom, "text");
    let owner = NodeKey::from_packed(10);
    let old = segment(owner, &node, false);
    let mut store = BindingStore::default();
    install(&mut store, std::slice::from_ref(&old), &[]);
    let stale = store.stage().into_update();
    install(&mut store, &[], &[]);
    assert!(
        store
            .prepare(stale, [], &Admission::default())
            .unwrap()
            .is_none()
    );
    let mut stage = store.stage();
    stage.replace(old.clone()).unwrap();
    let conflict = stage.into_update();
    assert!(
        store
            .prepare(conflict, [owner], &Admission::default())
            .unwrap()
            .is_none()
    );
    assert!(store.stage().resume(&old));
}

#[test]
fn nested_capture_is_direct_and_repeated_finish_does_not_pop_parent() {
    let dom = RcDom::default();
    let parent = text(&dom, "parent read");
    let child = text(&dom, "child read");
    let (a, b) = (NodeKey::from_packed(10), NodeKey::from_packed(11));
    let admission = Admission::default();
    let mut recorder = BindingRecorder::default();
    recorder.begin_owner(a);
    recorder.text(&parent);
    recorder.begin_owner(b);
    recorder.text(&child);
    let child_segment = recorder.finish_owner(b, &admission).unwrap();
    let repeat = recorder.finish_owner(b, &admission).unwrap();
    assert!(Rc::ptr_eq(&repeat, &child_segment));
    recorder.read(&parent);
    let parent_segment = recorder.finish_owner(a, &admission).unwrap();
    assert_eq!(child_segment.direct.len(), 1);
    assert_eq!(parent_segment.direct.len(), 1);
    assert_eq!(parent_segment.direct[0].0, parent.key());
    let mut store = BindingStore::default();
    let mut stage = store.stage();
    stage
        .recorded(recorder.into_recording(&admission).unwrap())
        .unwrap();
    let update = stage.into_update();
    store
        .prepare(update, [], &admission)
        .unwrap()
        .unwrap()
        .publish();
    assert!(store.was_read(parent.key(), Rc::as_ptr(&parent) as usize));
    assert!(!store.was_read(child.key(), Rc::as_ptr(&child) as usize));
}

#[test]
fn full_conversion_replaces_old_bindings() {
    let values = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let media = crate::style::css::Media::default();
    crate::dom::parse_into("<p>first</p>", "", media, false, &values).unwrap();
    let first = values.traversal.bindings().texts().next().unwrap().0;
    crate::dom::parse_into("<p>second</p>", "", media, false, &values).unwrap();
    assert_eq!(values.traversal.bindings().texts().count(), 1);
    // A fresh DOM may reuse the number, but its pointer and direct capability change.
    assert!(values.traversal.bindings().text(first).is_some());
    crate::dom::parse_into("<div></div>", "", media, false, &values).unwrap();
    assert!(values.traversal.bindings().text(first).is_none());
}

#[test]
fn a_fresh_store_cannot_accept_an_old_stores_same_generation_update() {
    let dom = RcDom::default();
    let node = text(&dom, "text");
    let owner = NodeKey::from_packed(10);
    let old = segment(owner, &node, false);
    let mut previous = BindingStore::default();
    install(&mut previous, std::slice::from_ref(&old), &[]);
    let foreign = previous.stage().into_update();
    let mut current = BindingStore::default();
    install(&mut current, std::slice::from_ref(&old), &[]);
    assert_eq!(previous.generation, current.generation);
    assert!(
        current
            .prepare(foreign, [], &Admission::default())
            .unwrap()
            .is_none()
    );
    let stale = current.stage().into_update();
    current = BindingStore::default();
    install(&mut current, std::slice::from_ref(&old), &[]);
    assert!(
        current
            .prepare(stale, [], &Admission::default())
            .unwrap()
            .is_none()
    );
    assert!(current.stage().resume(&old));
}
