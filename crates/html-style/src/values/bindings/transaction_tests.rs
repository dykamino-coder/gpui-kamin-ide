//! Binding ownership, admission, publication, and cancellation contracts.
use super::*;
use crate::budget::admission::Admission;
use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
use markup5ever_rcdom::{NodeData, RcDom};

#[test]
fn rejected_work_does_not_publish_or_remove_current_bindings() {
    let dom = RcDom::default();
    let node = dom.new_node(NodeData::Text {
        contents: std::cell::RefCell::new("text".into()),
    });
    let owner = NodeKey::from_packed(10);
    let mut recorder = BindingRecorder::default();
    recorder.begin_owner(owner);
    recorder.text(&node);
    let segment = recorder.end_owner(&Admission::default()).unwrap();
    let mut store = BindingStore::default();
    let mut stage = store.stage();
    stage.replace(segment.clone()).unwrap();
    let update = stage.into_update();
    store
        .prepare(update, [], &Admission::default())
        .unwrap()
        .unwrap()
        .publish();
    let limits = DocumentLimits::default()
        .lower(BudgetKind::TraversalVisits, 0)
        .unwrap();
    let budget = DocumentBudget::new(limits, ResourceGovernor::default()).unwrap();
    let admission = Admission::new(budget.revision());
    let update = store.stage().into_update();
    assert!(store.prepare(update, [owner], &admission).is_err());
    assert!(store.stage().resume(&segment));
    assert_eq!(
        store.text(TextId::new(node.key().packed()).unwrap()),
        Some(Rc::as_ptr(&node) as usize)
    );
}

#[test]
fn a_new_context_shares_only_bindings_and_aborted_recording_is_local() {
    use crate::{style::css::Media, values::DocumentValues};
    let old = DocumentValues::detached(Media::default());
    crate::dom::parse_into("<p>first</p>", "", Media::default(), false, &old).unwrap();
    let map = old.traversal.binding_store();
    let before: Vec<_> = map.borrow().texts().collect();
    let next = DocumentValues::detached(Media::default());
    assert!(next.traversal.attach_bindings(map.clone()).is_ok());
    assert_eq!(
        next.traversal.bindings().texts().collect::<Vec<_>>(),
        before
    );
    let weak = Rc::downgrade(&old);
    drop(old);
    assert!(weak.upgrade().is_none());
    next.traversal.begin_bindings(NodeKey::from_packed(100));
    drop(next);
    assert_eq!(map.borrow().texts().collect::<Vec<_>>(), before);
}

#[test]
#[should_panic(expected = "finished source owner")]
fn reentered_owner_cannot_return_an_earlier_finished_capability() {
    let mut recorder = BindingRecorder::default();
    let owner = NodeKey::from_packed(1);
    recorder.begin_owner(owner);
    recorder.finish_owner(owner, &Admission::default()).unwrap();
    recorder.begin_owner(owner);
    recorder.begin_owner(NodeKey::from_packed(2));
    recorder.finish_owner(owner, &Admission::default()).unwrap();
}

#[test]
fn raw_recording_joins_a_current_stage_without_changing_its_generation() {
    let dom = RcDom::default();
    let text = dom.new_node(NodeData::Text {
        contents: std::cell::RefCell::new("read".into()),
    });
    let owner = NodeKey::from_packed(10);
    let mut store = BindingStore::default();
    let stale = store.stage().into_update();
    let mut recorder = BindingRecorder::default();
    recorder.begin_owner(owner);
    recorder.text(&text);
    recorder.read(&text);
    let segment = recorder.end_owner(&Admission::default()).unwrap();
    let mut stage = store.stage();
    stage
        .recorded(recorder.into_recording(&Admission::default()).unwrap())
        .unwrap();
    let update = stage.into_update();
    store
        .prepare(update, [], &Admission::default())
        .unwrap()
        .unwrap()
        .publish();
    assert!(store.stage().resume(&segment));
    assert!(
        store
            .prepare(stale, [], &Admission::default())
            .unwrap()
            .is_none()
    );
    assert!(store.was_read(text.key(), Rc::as_ptr(&text) as usize));
}
