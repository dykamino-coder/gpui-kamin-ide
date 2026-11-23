//! Changed sequences invalidate positional matching throughout their output.
use super::*;
use crate::dom::source_delta::SourceChangeKind;

#[test]
fn document_sequence_has_output_dependencies_without_a_cascade_entry() {
    let (input, _values, mut cache, _) = build("<main id=m><p id=p>x</p></main>");
    let source = input.dom.document.key();
    let p = handle(&input, "p").key();
    assert!(cache.raw().get(source).is_none());
    assert_eq!(cache.raw().output().ownership.sequence, Some(source));
    let delta = stages::change(source, SourceChangeKind::Children, None, None);
    let stage = cache.raw_mut().stage(&delta);
    assert!(stage.dirty().directly_changes(p));
    assert!(stage.reusable(p, &stage.get(p).unwrap().inputs).is_none());
}

#[test]
fn reordered_sequence_rejects_sibling_and_descendant_reuse() {
    let (input, _values, mut cache, _) = build(
        "<main id=m><div id=p><span id=a><b id=c>x</b></span><span id=b>y</span></div><p id=q>z</p></main>",
    );
    let key = |id| handle(&input, id).key();
    let delta = stages::change(
        key("p"),
        SourceChangeKind::Children,
        Some(key("m")),
        Some(key("m")),
    );
    let stage = cache.raw_mut().stage(&delta);
    for id in ["p", "a", "b", "c"] {
        let node = key(id);
        assert!(stage.dirty().directly_changes(node));
        assert!(
            stage
                .reusable(node, &stage.get(node).unwrap().inputs)
                .is_none()
        );
    }
    assert!(stage.dirty().contains(key("m")));
    assert!(!stage.dirty().directly_changes(key("m")));
    let q = key("q");
    assert!(stage.reusable(q, &stage.get(q).unwrap().inputs).is_some());
}

#[test]
fn moved_sequence_invalidates_old_and_new_siblings_but_not_other_branches() {
    let (input, _values, mut cache, _) = build(
        "<main id=m><div id=p><i id=a>x</i><i id=c>y</i></div><div id=q><i id=b>z</i></div><p id=r>t</p></main>",
    );
    let key = |id| handle(&input, id).key();
    let delta = stages::change(
        key("c"),
        SourceChangeKind::Reparent,
        Some(key("p")),
        Some(key("q")),
    );
    let stage = cache.raw_mut().stage(&delta);
    for id in ["p", "q", "a", "b", "c"] {
        assert!(stage.dirty().directly_changes(key(id)));
    }
    assert!(!stage.dirty().contains(key("r")));
    assert!(!stage.dirty().directly_changes(key("m")));
}
