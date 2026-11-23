//! The real binding capability participates in the memo's prepared transaction.
use super::*;
use crate::dom::source_delta::{SourceChangeKind, SourceDelta};
use crate::values::bindings::{BindingStage, BindingStore};
use std::cell::RefCell;

struct CurrentBindings<'a> {
    stage: BindingStage<'a>,
    reused: Vec<NodeKey>,
}
impl ResumeBindings for CurrentBindings<'_> {
    fn resume(&mut self, entry: &CascadeEntry) -> Result<bool, Error> {
        let accepted = self.stage.resume(&entry.bindings);
        if accepted {
            self.reused.push(entry.node);
        }
        Ok(accepted)
    }
}

// Test-only orchestration: production remains closed until normalization and
// all publication consumers have been integrated and accepted together.
fn revise(
    input: &Admitted,
    cache: &mut RestyleCache,
    store: &Rc<RefCell<BindingStore>>,
    delta: &SourceDelta,
    publish: bool,
) -> (Rc<DocumentValues>, Vec<NodeKey>, Vec<NodeKey>) {
    let values = DocumentValues::detached(Media::default());
    values.admit_revision(input.context().budget().clone(), None);
    assert!(values.traversal.attach_bindings(store.clone()).is_ok());
    values.traversal.begin_bindings(NodeKey::NONE);
    let probe = cache
        .probe(input, "", Media::default(), &values)
        .unwrap()
        .unwrap();
    let prepared = cache.prepare_cascade(&probe, input, &values).unwrap();
    let current = store.borrow();
    let mut bindings = CurrentBindings {
        stage: current.stage(),
        reused: Vec::new(),
    };
    let mut memo = cache.raw_mut().stage(delta);
    let output = memo.cascade(prepared.input(), &mut bindings).unwrap();
    let admission = Admission::new(input.context().budget().clone());
    values
        .traversal
        .end_bindings(NodeKey::NONE, &admission)
        .unwrap();
    bindings
        .stage
        .recorded(values.traversal.take_binding_recording(&admission).unwrap())
        .unwrap();
    let update = bindings.stage.into_update();
    let reused = bindings.reused;
    drop(current);
    let memo = memo.prepare(output).unwrap();
    let retired = memo.retired().iter().copied().collect();
    let mut current = store.borrow_mut();
    let bindings = current
        .prepare(update, memo.retired().iter().copied(), &admission)
        .unwrap()
        .unwrap();
    if publish {
        bindings.publish();
        memo.publish();
    }
    (values, reused, retired)
}

#[test]
fn real_bindings_and_raw_entries_abort_together_then_publish_a_text_edit() {
    let (input, old_values, mut cache, _) = build(
        "<!doctype html><main id=m><p id=p>before</p><aside id=q style='width:calc(10px + 20%)'>clean</aside></main>",
    );
    let p = handle(&input, "p");
    let q = handle(&input, "q").key();
    let text = p.children.borrow()[0].clone();
    let store = old_values.traversal.binding_store();
    let before_p = cache.raw().get(p.key()).unwrap().clone();
    let before_q = cache.raw().get(q).unwrap().clone();
    let roots = cache.raw().roots().identity();
    let weak = Rc::downgrade(&old_values);
    drop(old_values);
    assert!(weak.upgrade().is_none());
    let NodeData::Text { contents } = &text.data else {
        panic!("text")
    };
    *contents.borrow_mut() = "after".into();
    let delta = stages::change(
        text.key(),
        SourceChangeKind::CharacterData,
        Some(p.key()),
        Some(p.key()),
    );
    let (aborted, reused, retired) = revise(&input, &mut cache, &store, &delta, false);
    assert!(reused.contains(&q));
    assert!(retired.is_empty());
    drop(aborted);
    assert_eq!(cache.raw().roots().identity(), roots);
    assert!(Rc::ptr_eq(cache.raw().get(p.key()).unwrap(), &before_p));
    assert!(Rc::ptr_eq(
        &store.borrow().segment(p.key()).unwrap(),
        &before_p.bindings
    ));
    let (values, reused, retired) = revise(&input, &mut cache, &store, &delta, true);
    assert!(reused.contains(&q));
    assert!(retired.is_empty());
    assert!(Rc::ptr_eq(cache.raw().get(q).unwrap(), &before_q));
    assert!(Rc::ptr_eq(
        &store.borrow().segment(q).unwrap(),
        &before_q.bindings
    ));
    assert!(!Rc::ptr_eq(cache.raw().get(p.key()).unwrap(), &before_p));
    assert!(Rc::ptr_eq(
        &store.borrow().segment(p.key()).unwrap(),
        &cache.raw().get(p.key()).unwrap().bindings
    ));
    super::traverse::assert_fresh(&input, &cache);
    drop(values);
}

#[test]
fn memo_retirement_is_the_only_removal_set_for_text_bindings() {
    let (input, old_values, mut cache, _) = build(
        "<!doctype html><main id=m><div id=p><span id=c>removed</span></div><aside id=q>clean</aside></main>",
    );
    let c = handle(&input, "c");
    let p = handle(&input, "p");
    let q = handle(&input, "q").key();
    let text = c.children.borrow()[0].clone();
    let text_id = crate::dom::TextId::new(text.key().packed()).unwrap();
    let store = old_values.traversal.binding_store();
    let before_c = cache.raw().get(c.key()).unwrap().clone();
    let before_q = cache.raw().get(q).unwrap().clone();
    assert!(store.borrow().text(text_id).is_some());
    retirement::detach(&c);
    let delta = stages::change(c.key(), SourceChangeKind::Reparent, Some(p.key()), None);
    let (aborted, _, retired) = revise(&input, &mut cache, &store, &delta, false);
    assert!(retired.contains(&c.key()));
    drop(aborted);
    assert!(Rc::ptr_eq(cache.raw().get(c.key()).unwrap(), &before_c));
    assert!(store.borrow().text(text_id).is_some());
    let (_values, reused, retired) = revise(&input, &mut cache, &store, &delta, true);
    assert!(retired.contains(&c.key()));
    assert!(reused.contains(&q));
    assert!(cache.raw().get(c.key()).is_none());
    assert!(store.borrow().segment(c.key()).is_none());
    assert!(store.borrow().text(text_id).is_none());
    assert!(Rc::ptr_eq(cache.raw().get(q).unwrap(), &before_q));
    super::traverse::assert_fresh(&input, &cache);
}
