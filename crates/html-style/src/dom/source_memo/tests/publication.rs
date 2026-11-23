//! A later consumer can reject prepared retirement without changing the memo.
use super::*;
use crate::dom::source_delta::SourceDelta;
use std::collections::HashSet;

#[test]
fn aborted_preparation_keeps_entries_and_readers_and_can_be_retried() {
    let (input, _values, mut cache, _) =
        build("<main id=m><div id=p><b id=c>x</b></div><p id=q>y</p></main>");
    let p = handle(&input, "p");
    let c = handle(&input, "c").key();
    let old_entries = cache.raw().entries.clone();
    let old_roots = cache.raw().roots().identity();
    let old_readers: Vec<_> = old_entries
        .values()
        .filter_map(|entry| entry.ownership.sequence)
        .map(|sequence| (sequence, cache.raw().readers(sequence).to_vec()))
        .collect();
    let parent = retirement::detach(&p);
    let (_new_values, fresh, _) = recascade(&input);
    let replacements = retirement::path_entries(fresh.raw(), &[parent.key()]);
    let mut stage = cache.raw_mut().stage(&SourceDelta::default());
    for entry in replacements {
        stage.replace(entry).unwrap();
    }
    let prepared = stage.prepare(fresh.raw().output().clone()).unwrap();
    assert_eq!(prepared.retired(), &HashSet::from([p.key(), c]));
    // A binding consumer may reject its own preparation here.
    drop(prepared);
    assert_eq!(cache.raw().len(), old_entries.len());
    assert_eq!(cache.raw().roots().identity(), old_roots);
    for (key, entry) in &old_entries {
        assert!(Rc::ptr_eq(cache.raw().get(*key).unwrap(), entry));
    }
    for (sequence, readers) in old_readers {
        assert_eq!(cache.raw().readers(sequence), readers);
    }

    let mut update = MemoUpdate::default();
    for entry in retirement::path_entries(fresh.raw(), &[parent.key()]) {
        update.replace(entry).unwrap();
    }
    let prepared = cache
        .raw_mut()
        .prepare(update, fresh.raw().output().clone())
        .unwrap();
    assert_eq!(prepared.retired(), &HashSet::from([p.key(), c]));
    prepared.publish();
    retirement::assert_graph(cache.raw(), fresh.raw());
    assert!(cache.raw().get(p.key()).is_none());
    assert!(cache.raw().get(c).is_none());
    assert!(cache.raw().readers(p.key()).is_empty());
    assert!(cache.raw().readers(c).is_empty());
}
