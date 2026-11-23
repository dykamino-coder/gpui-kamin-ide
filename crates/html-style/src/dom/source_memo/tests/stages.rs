//! Dirty paths, retirement and abort preserve stable source entries.
use super::*;
use crate::dom::source_delta::{SourceChange, SourceChangeKind, SourceDelta};

pub(super) fn change(
    node: NodeKey,
    kind: SourceChangeKind,
    before: Option<NodeKey>,
    after: Option<NodeKey>,
) -> SourceDelta {
    SourceDelta {
        changes: vec![SourceChange {
            node,
            kind,
            before_parent: before,
            after_parent: after,
        }],
    }
}

#[test]
fn text_dirties_its_parent_sequence_and_only_its_ancestor_path() {
    let (input, _values, mut cache, _) =
        build("<main id=m><div id=p>x</div><div id=q>y</div></main>");
    let p = handle(&input, "p");
    let text = p.children.borrow()[0].key();
    let delta = change(
        text,
        SourceChangeKind::CharacterData,
        Some(p.key()),
        Some(p.key()),
    );
    let stage = cache.raw_mut().stage(&delta);
    assert!(stage.dirty().directly_changes(p.key()));
    assert!(stage.dirty().contains(handle(&input, "m").key()));
    assert!(!stage.dirty().directly_changes(handle(&input, "m").key()));
    assert!(!stage.dirty().contains(handle(&input, "q").key()));
}

#[test]
fn moving_a_subtree_dirties_old_and_new_paths() {
    let (input, _values, mut cache, _) =
        build("<main id=m><div id=p><span id=c>text</span></div><div id=q></div></main>");
    let p = handle(&input, "p").key();
    let q = handle(&input, "q").key();
    let c = handle(&input, "c").key();
    let delta = change(c, SourceChangeKind::Reparent, Some(p), Some(q));
    let stage = cache.raw_mut().stage(&delta);
    assert!(stage.dirty().contains(p));
    assert!(stage.dirty().contains(q));
}

#[test]
fn repeated_publication_keeps_one_current_entry_and_a_held_old_frame() {
    let (input, _values, mut cache, _) = build("<p id=p>x</p><p id=q>y</p>");
    let p = handle(&input, "p").key();
    let q = handle(&input, "q").key();
    let held = cache.raw().get(p).unwrap().clone();
    let untouched = cache.raw().get(q).unwrap().clone();
    let roots = cache.raw().output().clone();
    let count = cache.raw().len();
    let budget = input.context().budget().document().clone();
    let live = budget.used(crate::budget::BudgetKind::LiveBytes);
    let admission = Admission::new(input.context().budget().clone());
    let mut steady = None;
    for _ in 0..10_000 {
        let replacement = CascadeEntry::new(
            p,
            held.source_parent,
            held.ownership.clone(),
            held.inputs.clone(),
            CapturedOutput {
                raw: held.raw.clone(),
                effects: held.effects.clone(),
                bindings: held.bindings.clone(),
            },
            &admission,
        )
        .unwrap();
        let weak = Rc::downgrade(cache.raw().get(p).unwrap());
        let mut stage = cache.raw_mut().stage(&SourceDelta::default());
        stage.replace(replacement).unwrap();
        stage.commit(roots.clone()).unwrap();
        if !weak.ptr_eq(&Rc::downgrade(&held)) {
            assert!(weak.upgrade().is_none());
        }
        assert_eq!(cache.raw().len(), count);
        let used = budget.used(crate::budget::BudgetKind::LiveBytes);
        assert_eq!(*steady.get_or_insert(used), used);
    }
    assert!(Rc::ptr_eq(cache.raw().get(q).unwrap(), &untouched));
    assert_eq!(Rc::strong_count(&held), 1);
    drop(held);
    assert_eq!(budget.used(crate::budget::BudgetKind::LiveBytes), live);
}

#[test]
fn reuse_requires_all_entry_inputs_and_no_dirty_descendants() {
    let (input, values, mut cache, _) = build("<div id=p><span id=c>x</span></div><p id=q>y</p>");
    let p = handle(&input, "p").key();
    let c = handle(&input, "c").key();
    let q = handle(&input, "q").key();
    let inputs = cache.raw().get(q).unwrap().inputs.clone();
    let delta = change(c, SourceChangeKind::Attributes(vec![]), Some(p), Some(p));
    let stage = cache.raw_mut().stage(&delta);
    assert!(stage.reusable(q, &inputs).is_some());
    let mut changed = inputs.clone();
    changed.depth += 1;
    assert!(stage.reusable(q, &changed).is_none());
    changed = inputs.clone();
    let mut vcx = values.values();
    vcx.viewport.0 += 100.0;
    changed.environment = Rc::new(crate::style::cascade::sharing::Environment::new(vcx));
    assert!(stage.reusable(q, &changed).is_none());
    changed = inputs;
    changed.counters.reset("number", 7);
    assert!(stage.reusable(q, &changed).is_none());
    assert!(stage.reusable(p, &stage.get(p).unwrap().inputs).is_none());
    assert!(!stage.dirty().directly_changes(p));
}
