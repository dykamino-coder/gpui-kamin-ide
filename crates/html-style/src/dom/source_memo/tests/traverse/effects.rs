//! Skipped segment effects become the inputs of the next dirty branch.
use super::*;

#[test]
fn clean_counter_segment_restores_the_input_of_a_dirty_following_sibling() {
    let (input, values, mut cache, _) = build(
        "<!doctype html><ol id=o><li id=a style='counter-increment:list-item 3'></li><li id=b style='color:red'></li></ol>",
    );
    let a = handle(&input, "a").key();
    let b = handle(&input, "b");
    let old_a = cache.raw().get(a).unwrap().clone();
    let expected = cache.raw().get(b.key()).unwrap().inputs.counters.clone();
    set_style(&b, "color:blue");
    let mut bindings = Bindings::default();
    run(
        &input,
        &values,
        &mut cache,
        &change(
            b.key(),
            SourceChangeKind::Attributes(Vec::new()),
            None,
            None,
        ),
        &mut bindings,
    )
    .unwrap();
    assert!(bindings.reused.contains(&a));
    assert!(Rc::ptr_eq(cache.raw().get(a).unwrap(), &old_a));
    assert!(cache.raw().get(b.key()).unwrap().inputs.counters == expected);
    assert_fresh(&input, &cache);
}

#[test]
fn a_clean_document_root_restores_root_metrics_without_entering_descendants() {
    let (input, values, mut cache, _) = build(
        "<!doctype html><html id=r style='font-size:2rem;line-height:3'><body><p id=p></p></body></html>",
    );
    let root = handle(&input, "r").key();
    let old = cache.raw().get(root).unwrap().clone();
    let expected = values.root.borrow().clone();
    let mut bindings = Bindings::default();
    run(
        &input,
        &values,
        &mut cache,
        &SourceDelta::default(),
        &mut bindings,
    )
    .unwrap();
    assert_eq!(bindings.reused, [root]);
    assert!(Rc::ptr_eq(cache.raw().get(root).unwrap(), &old));
    assert!(values.root.borrow().same_inputs(&expected));
    assert_fresh(&input, &cache);
}

#[test]
fn a_clean_first_link_selects_the_current_scroll_group_before_a_dirty_link() {
    let (input, values, mut cache, _) = build(
        "<!doctype html><nav id=g style='scroll-target-group:auto;counter-reset:n'><a id=a href='#x' style='counter-increment:n'></a><a id=b href='#y' style='color:red'></a></nav><p id=x></p><p id=y></p>",
    );
    let a = handle(&input, "a").key();
    let b = handle(&input, "b");
    assert!(cache.raw().get(a).unwrap().effects.selects_group.is_some());
    set_style(&b, "color:blue");
    let mut bindings = Bindings::default();
    run(
        &input,
        &values,
        &mut cache,
        &change(
            b.key(),
            SourceChangeKind::Attributes(Vec::new()),
            None,
            None,
        ),
        &mut bindings,
    )
    .unwrap();
    assert!(bindings.reused.contains(&a));
    assert!(
        cache
            .raw()
            .get(b.key())
            .unwrap()
            .inputs
            .group
            .unwrap()
            .selected
    );
    assert_fresh(&input, &cache);
}

#[test]
fn changing_the_current_link_recascades_its_descendant_selector() {
    // The style text is not rendered; the fixture has no source-text output bindings.
    let (input, values, mut cache, _) = build(
        "<!doctype html><style>a:is(:target-current) i{color:red}</style><nav id=g style='scroll-target-group:auto'><a id=a href='#x'></a><a id=b href='#y'><i id=c></i></a></nav><p id=x></p><p id=y></p>",
    );
    let a = handle(&input, "a");
    let b = handle(&input, "b").key();
    let c = handle(&input, "c").key();
    let old_c = cache.raw().get(c).unwrap().clone();
    let NodeData::Element { attrs, .. } = &a.data else {
        panic!("link")
    };
    attrs
        .borrow_mut()
        .retain(|attr| &*attr.name.local != "href");
    let mut bindings = Bindings::default();
    run(
        &input,
        &values,
        &mut cache,
        &change(
            a.key(),
            SourceChangeKind::Attributes(Vec::new()),
            None,
            None,
        ),
        &mut bindings,
    )
    .unwrap();
    assert!(cache.raw().get(b).unwrap().effects.selects_group.is_some());
    assert!(!Rc::ptr_eq(cache.raw().get(c).unwrap(), &old_c));
    let (Node::Element(before), Node::Element(after)) =
        (&old_c.raw[0], &cache.raw().get(c).unwrap().raw[0])
    else {
        panic!("child element")
    };
    assert!(!crate::dom::same_cascaded_tree(
        before,
        after,
        false,
        &mut CascadedMemo::default(),
    ));
    assert_fresh(&input, &cache);
}
