//! Publication reconciles removed edges only after all relocation claims.
use super::*;
use crate::dom::source_delta::SourceDelta;
use std::collections::HashSet;

pub(super) fn detach(node: &Handle) -> Handle {
    let parent = node.parent.take().unwrap().upgrade().unwrap();
    parent
        .children
        .borrow_mut()
        .retain(|n| !Rc::ptr_eq(n, node));
    parent
}

pub(super) fn path_entries(memo: &SourceMemo, changed: &[NodeKey]) -> Vec<Rc<CascadeEntry>> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for node in changed {
        let mut next = Some(*node);
        while let Some(key) = next {
            if !seen.insert(key) {
                break;
            }
            let entry = memo.get(key).unwrap();
            entries.push(entry.clone());
            next = entry.ownership.parent;
        }
    }
    entries
}

pub(super) fn assert_graph(actual: &SourceMemo, expected: &SourceMemo) {
    assert_eq!(actual.len(), expected.len());
    for (node, entry) in &expected.entries {
        let got = actual.get(*node).unwrap();
        assert_eq!(got.source_parent, entry.source_parent);
        assert_eq!(got.ownership.parent, entry.ownership.parent);
        assert_eq!(got.ownership.children, entry.ownership.children);
        assert_eq!(got.ownership.sequence, entry.ownership.sequence);
        assert_eq!(got.raw.len(), entry.raw.len());
        if let Some(sequence) = entry.ownership.sequence {
            let readers = |memo: &SourceMemo| {
                memo.readers(sequence)
                    .iter()
                    .copied()
                    .collect::<HashSet<_>>()
            };
            assert_eq!(readers(actual), readers(expected));
        }
    }
    assert_eq!(
        actual.output.ownership.children,
        expected.output.ownership.children
    );
}

#[test]
fn hiding_a_parent_retires_its_unvisited_descendants_without_an_explicit_retire() {
    let (input, _values, mut cache, _) =
        build("<main id=m><div id=p><span id=c>x</span></div><p id=q>y</p></main>");
    let p = handle(&input, "p");
    let c = handle(&input, "c").key();
    let q = handle(&input, "q").key();
    let untouched = cache.raw().get(q).unwrap().clone();
    let held = cache.raw().get(c).unwrap().clone();
    let NodeData::Element { attrs, .. } = &p.data else {
        unreachable!()
    };
    attrs.borrow_mut().push(html5ever::Attribute {
        name: html5ever::QualName::new(None, html5ever::ns!(), "style".into()),
        value: "display:none".into(),
    });
    let (_new_values, fresh, _) = recascade(&input);
    let replacements = path_entries(fresh.raw(), &[p.key()]);
    let mut stage = cache.raw_mut().stage(&SourceDelta::default());
    for entry in replacements {
        stage.replace(entry).unwrap();
    }
    stage.commit(fresh.raw().output().clone()).unwrap();
    assert_graph(cache.raw(), fresh.raw());
    assert!(cache.raw().get(c).is_none());
    assert!(!held.raw.is_empty());
    assert!(Rc::ptr_eq(cache.raw().get(q).unwrap(), &untouched));
}

#[test]
fn moving_a_descendant_out_of_a_removed_parent_is_independent_of_staging_order() {
    for reverse in [false, true] {
        let (input, _values, mut cache, _) = build(
            "<main id=m><div id=p><b id=d>gone<span id=c><i id=k>kept</i></span></b></div>\
             <div id=q></div><p id=r>untouched</p></main>",
        );
        let p = handle(&input, "p");
        let c = handle(&input, "c");
        let q = handle(&input, "q");
        let d = handle(&input, "d").key();
        let k = handle(&input, "k").key();
        let r = handle(&input, "r").key();
        let untouched = cache.raw().get(r).unwrap().clone();
        let old = cache.raw().get(c.key()).unwrap().clone();
        let old_child = cache.raw().get(k).unwrap().clone();
        detach(&c);
        c.parent.set(Some(markup5ever_rcdom::WeakHandle::new(&q)));
        q.children.borrow_mut().push(c.clone());
        let parent = detach(&p);
        let (_new_values, fresh, _) = recascade(&input);
        let mut replacements = path_entries(fresh.raw(), &[parent.key(), c.key()]);
        if reverse {
            replacements.reverse();
        }
        let mut stage = cache.raw_mut().stage(&SourceDelta::default());
        for entry in replacements {
            stage.replace(entry).unwrap();
        }
        stage.commit(fresh.raw().output().clone()).unwrap();
        assert_graph(cache.raw(), fresh.raw());
        assert!(cache.raw().get(p.key()).is_none());
        assert!(cache.raw().get(d).is_none());
        assert!(Rc::ptr_eq(cache.raw().get(k).unwrap(), &old_child));
        assert_eq!(
            cache.raw().get(c.key()).unwrap().source_parent,
            Some(q.key())
        );
        assert_eq!(old.source_parent, Some(d));
        assert!(Rc::ptr_eq(cache.raw().get(r).unwrap(), &untouched));
    }
}

#[test]
fn abort_keeps_old_entries_raw_roots_and_reverse_edges() {
    let (input, _values, mut cache, _) = build("<main id=m><div id=p><b id=c>x</b></div></main>");
    let p = handle(&input, "p");
    let old = cache.raw().get(p.key()).unwrap().clone();
    let roots = cache.raw().roots().identity();
    let count = cache.raw().len();
    let parent = detach(&p);
    let (_new_values, fresh, _) = recascade(&input);
    let entries = path_entries(fresh.raw(), &[parent.key()]);
    let staged = Rc::downgrade(&entries[0]);
    {
        let mut stage = cache.raw_mut().stage(&SourceDelta::default());
        for entry in entries {
            stage.replace(entry).unwrap();
        }
        assert!(stage.get(p.key()).is_some());
    }
    drop(fresh);
    assert!(staged.upgrade().is_none());
    assert!(Rc::ptr_eq(cache.raw().get(p.key()).unwrap(), &old));
    assert_eq!(cache.raw().roots().identity(), roots);
    assert_eq!(cache.raw().len(), count);
    assert!(cache.raw().readers(p.key()).contains(&p.key()));
}

#[test]
fn removing_document_output_retires_all_old_roots_and_reverse_edges() {
    let (input, _values, mut cache, _) = build("<main id=m><p id=p>x</p></main>");
    let sequences: Vec<_> = cache
        .raw()
        .entries
        .values()
        .filter_map(|e| e.ownership.sequence)
        .collect();
    input.dom.document.children.borrow_mut().clear();
    let (_new_values, fresh, _) = recascade(&input);
    cache
        .raw_mut()
        .stage(&SourceDelta::default())
        .commit(fresh.raw().output().clone())
        .unwrap();
    assert_graph(cache.raw(), fresh.raw());
    assert!(cache.raw().is_empty());
    for sequence in sequences {
        assert!(cache.raw().readers(sequence).is_empty());
    }
}
