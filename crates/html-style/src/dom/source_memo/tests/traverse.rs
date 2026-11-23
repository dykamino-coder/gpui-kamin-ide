//! Real enter/leave skips clean segments and preserves transactional memo publication.
use super::stages::change;
use super::*;
use crate::budget::admission::Error;
use crate::dom::{
    CascadedMemo,
    source_delta::{SourceChangeKind, SourceDelta},
};

mod effects;
mod prepared;

// These fixtures use one value owner and no source text. The cross-revision
// adapter must separately provide owning values and transactional bindings.
#[derive(Default)]
struct Bindings {
    reused: Vec<NodeKey>,
    reject: bool,
    fail: Option<NodeKey>,
}
impl ResumeBindings for Bindings {
    fn resume(&mut self, entry: &CascadeEntry) -> Result<bool, Error> {
        if self.fail == Some(entry.node) {
            return Err(Error::Allocation);
        }
        if self.reject {
            return Ok(false);
        }
        self.reused.push(entry.node);
        Ok(true)
    }
}

fn set_style(node: &Handle, style: &str) {
    let NodeData::Element { attrs, .. } = &node.data else {
        panic!("element")
    };
    attrs
        .borrow_mut()
        .iter_mut()
        .find(|a| &*a.name.local == "style")
        .unwrap()
        .value = style.into();
}

fn run(
    input: &Admitted,
    values: &Rc<DocumentValues>,
    cache: &mut RestyleCache,
    delta: &SourceDelta,
    bindings: &mut Bindings,
) -> Result<(), Error> {
    // The orchestrator owns staged ValueCx; this fixture resets initial root inputs.
    *values.root.borrow_mut() = DocumentValues::detached(Media::default())
        .root
        .borrow()
        .clone();
    let probe = cache
        .probe(input, "", Media::default(), values)
        .unwrap()
        .unwrap();
    let prepared = cache.prepare_cascade(&probe, input, values).unwrap();
    let mut stage = cache.raw_mut().stage(delta);
    let output = stage.cascade(prepared.input(), bindings)?;
    stage.commit(output)
}

fn assert_fresh(input: &Admitted, actual: &RestyleCache) {
    let (_values, fresh, _) = recascade(input);
    super::retirement::assert_graph(actual.raw(), fresh.raw());
    let mut compared = CascadedMemo::default();
    for (key, expected) in &fresh.raw().entries {
        let actual = actual.raw().get(*key).unwrap();
        for (a, b) in actual.raw.iter().zip(expected.raw.iter()) {
            match (a, b) {
                (Node::Element(a), Node::Element(b)) => assert!(
                    crate::dom::same_cascaded_tree(a, b, false, &mut compared),
                    "source {key:?}"
                ),
                (Node::Text(a), Node::Text(b)) => assert_eq!(&**a, &**b),
                _ => panic!("raw node kind"),
            }
        }
    }
}

#[test]
fn dirty_attribute_rebuilds_path_and_reuses_whole_clean_sibling() {
    let (input, values, mut cache, _) = build(
        "<!doctype html><main id=m><div id=p style='color:red'><i id=c></i></div><section id=q><b id=d></b></section></main>",
    );
    let p = handle(&input, "p");
    let q = handle(&input, "q").key();
    let d = handle(&input, "d").key();
    let old_q = cache.raw().get(q).unwrap().clone();
    let old_d = cache.raw().get(d).unwrap().clone();
    set_style(&p, "color:blue");
    let delta = change(
        p.key(),
        SourceChangeKind::Attributes(Vec::new()),
        Some(handle(&input, "m").key()),
        Some(handle(&input, "m").key()),
    );
    let mut bindings = Bindings::default();
    run(&input, &values, &mut cache, &delta, &mut bindings).unwrap();
    assert!(bindings.reused.contains(&q));
    assert!(!bindings.reused.contains(&d));
    assert!(Rc::ptr_eq(cache.raw().get(q).unwrap(), &old_q));
    assert!(Rc::ptr_eq(cache.raw().get(d).unwrap(), &old_d));
    assert_fresh(&input, &cache);
}

#[test]
fn changed_counter_exit_recalculates_following_siblings() {
    let (input, values, mut cache, _) = build(
        "<!doctype html><ol id=o><li id=a style='counter-increment:list-item 1'></li><li id=b></li></ol><aside id=q></aside>",
    );
    let a = handle(&input, "a");
    let b = handle(&input, "b").key();
    let old = cache.raw().get(b).unwrap().clone();
    set_style(&a, "counter-increment:list-item 3");
    let parent = handle(&input, "o").key();
    run(
        &input,
        &values,
        &mut cache,
        &change(
            a.key(),
            SourceChangeKind::Attributes(Vec::new()),
            Some(parent),
            Some(parent),
        ),
        &mut Bindings::default(),
    )
    .unwrap();
    assert!(!Rc::ptr_eq(cache.raw().get(b).unwrap(), &old));
    assert!(cache.raw().get(b).unwrap().inputs.counters != old.inputs.counters);
    assert_fresh(&input, &cache);
}

#[test]
fn moved_output_claims_survive_real_cascade_in_both_visit_orders() {
    for reversed in [false, true] {
        let source = if reversed {
            "<main id=m><div id=q></div><div id=p><i id=c></i></div></main>"
        } else {
            "<main id=m><div id=p><i id=c></i></div><div id=q></div></main>"
        };
        let (input, values, mut cache, _) = build(source);
        let c = handle(&input, "c");
        let q = handle(&input, "q");
        let p = super::retirement::detach(&c);
        c.parent.set(Some(markup5ever_rcdom::WeakHandle::new(&q)));
        q.children.borrow_mut().push(c.clone());
        let delta = change(
            c.key(),
            SourceChangeKind::Reparent,
            Some(p.key()),
            Some(q.key()),
        );
        run(
            &input,
            &values,
            &mut cache,
            &delta,
            &mut Bindings::default(),
        )
        .unwrap();
        assert_eq!(
            cache.raw().get(c.key()).unwrap().ownership.parent,
            Some(q.key())
        );
        assert_fresh(&input, &cache);
    }
}

#[test]
fn binding_failure_aborts_after_dirty_work_without_publishing_entries() {
    let (input, values, mut cache, _) = build("<div id=p style='color:red'></div><div id=q></div>");
    let p = handle(&input, "p");
    let old = cache.raw().get(p.key()).unwrap().clone();
    let roots = cache.raw().roots().identity();
    set_style(&p, "color:blue");
    let delta = change(
        p.key(),
        SourceChangeKind::Attributes(Vec::new()),
        None,
        None,
    );
    assert!(
        run(
            &input,
            &values,
            &mut cache,
            &delta,
            &mut Bindings {
                fail: Some(handle(&input, "q").key()),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(Rc::ptr_eq(cache.raw().get(p.key()).unwrap(), &old));
    assert_eq!(cache.raw().roots().identity(), roots);
}

#[test]
fn declined_bindings_keep_the_same_cascade_and_publication_algorithm() {
    let (input, values, mut cache, _) = build("<!doctype html><div id=p><i id=c></i></div>");
    let c = handle(&input, "c").key();
    let old = cache.raw().get(c).unwrap().clone();
    let mut bindings = Bindings {
        reject: true,
        ..Default::default()
    };
    run(
        &input,
        &values,
        &mut cache,
        &SourceDelta::default(),
        &mut bindings,
    )
    .unwrap();
    assert!(!Rc::ptr_eq(cache.raw().get(c).unwrap(), &old));
    assert!(bindings.reused.is_empty());
    assert_fresh(&input, &cache);
}
