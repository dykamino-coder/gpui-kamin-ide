//! Shadow and slot containment participates in dirty paths and atomic retirement.
use super::stages::change;
use super::*;
use crate::dom::source_delta::{SourceChangeKind, SourceDelta};

fn fragment(input: &Admitted, id: &str) -> Handle {
    let template = handle(input, id);
    let NodeData::Element {
        template_contents, ..
    } = &template.data
    else {
        unreachable!()
    };
    template_contents.borrow().as_ref().unwrap().clone()
}

#[test]
fn slots_keep_source_matching_and_output_parents_distinct() {
    let (input, _values, mut cache, _) = build(
        "<div id=h><template id=t shadowrootmode=open><section id=s><slot id=k></slot>\
         </section></template><p id=l>light</p></div><p id=q>other</p>",
    );
    let host = handle(&input, "h").key();
    let slot = handle(&input, "k").key();
    let light = handle(&input, "l").key();
    let entry = cache.raw().get(light).unwrap();
    assert_eq!(entry.source_parent, Some(host));
    assert_eq!(entry.inputs.parent.as_ref().unwrap().node, host);
    assert_eq!(entry.ownership.parent, Some(slot));
    assert_eq!(
        cache.raw().get(slot).unwrap().ownership.sequence,
        Some(host)
    );
    assert!(cache.raw().readers(host).contains(&slot));
    assert_eq!(
        cache.raw().get(host).unwrap().ownership.sequence,
        Some(fragment(&input, "t").key())
    );

    let delta = change(
        light,
        SourceChangeKind::Attributes(vec![]),
        Some(host),
        Some(host),
    );
    let stage = cache.raw_mut().stage(&delta);
    for id in ["h", "s", "k", "l"] {
        assert!(stage.dirty().contains(handle(&input, id).key()), "{id}");
    }
    assert!(!stage.dirty().contains(handle(&input, "q").key()));
}

#[test]
fn a_text_only_shadow_sequence_dirties_its_output_owner() {
    let (input, _values, mut cache, _) = build(
        "<div id=h><template id=t shadowrootmode=open>text</template></div><p id=q>other</p>",
    );
    let root = fragment(&input, "t");
    let host = handle(&input, "h").key();
    assert!(cache.raw().get(root.key()).is_none());
    assert_eq!(cache.raw().readers(root.key()), [host]);
    let delta = change(
        root.children.borrow()[0].key(),
        SourceChangeKind::CharacterData,
        Some(root.key()),
        Some(root.key()),
    );
    let stage = cache.raw_mut().stage(&delta);
    assert!(stage.dirty().directly_changes(host));
    assert!(!stage.dirty().contains(handle(&input, "q").key()));
}

#[test]
fn retiring_a_host_removes_shadow_and_slotted_entries_and_reverse_edges() {
    let (input, _values, mut cache, _) = build(
        "<div id=h><template id=t shadowrootmode=open><div id=s><slot id=k></slot></div>\
         </template><p id=l>light</p></div><p id=q>other</p>",
    );
    let host = handle(&input, "h").key();
    let root = fragment(&input, "t").key();
    let other = cache.raw().get(handle(&input, "q").key()).unwrap().clone();
    let removed: Vec<_> = ["h", "s", "k", "l"]
        .map(|id| handle(&input, id).key())
        .into();
    let parent = retirement::detach(&handle(&input, "h"));
    let (_new_values, fresh, _) = recascade(&input);
    let replacements = retirement::path_entries(fresh.raw(), &[parent.key()]);
    {
        let mut aborted = cache.raw_mut().stage(&SourceDelta::default());
        for entry in &replacements {
            aborted.replace(entry.clone()).unwrap();
        }
    }
    assert!(cache.raw().readers(root).contains(&host));
    let mut stage = cache.raw_mut().stage(&SourceDelta::default());
    for entry in replacements {
        stage.replace(entry).unwrap();
    }
    stage.commit(fresh.raw().output().clone()).unwrap();
    retirement::assert_graph(cache.raw(), fresh.raw());
    for key in removed {
        assert!(cache.raw().get(key).is_none());
    }
    assert!(cache.raw().readers(root).is_empty());
    assert!(cache.raw().readers(host).is_empty());
    assert!(Rc::ptr_eq(
        cache.raw().get(handle(&input, "q").key()).unwrap(),
        &other
    ));
}

#[test]
fn staged_sequence_changes_publish_only_new_reverse_edges() {
    let (input, _values, mut cache, _) = build("<p id=p>x</p><p id=q>y</p>");
    let p = handle(&input, "p").key();
    let q = handle(&input, "q").key();
    let old = cache.raw().get(p).unwrap().clone();
    let admission = Admission::new(input.context().budget().clone());
    let replacement = CascadeEntry::new(
        p,
        old.source_parent,
        OutputOwnership {
            sequence: Some(q),
            ..old.ownership.clone()
        },
        old.inputs.clone(),
        CapturedOutput {
            raw: old.raw.clone(),
            effects: old.effects.clone(),
            bindings: old.bindings.clone(),
        },
        &admission,
    )
    .unwrap();
    let roots = cache.raw().output().clone();
    let mut stage = cache.raw_mut().stage(&SourceDelta::default());
    stage.replace(replacement).unwrap();
    stage.commit(roots).unwrap();
    assert!(!cache.raw().readers(p).contains(&p));
    assert!(cache.raw().readers(q).contains(&p));
    assert!(cache.raw().readers(q).contains(&q));
}
