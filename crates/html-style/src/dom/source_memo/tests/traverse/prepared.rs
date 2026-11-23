//! Current matching marks and page scheme feed the same retained walk as probes.
use super::*;

#[test]
fn relational_mutation_prepares_current_marks_and_keeps_an_unaffected_segment() {
    relational_mutation(true);
}

#[test]
fn first_relational_match_also_refreshes_the_common_ancestor_marks() {
    relational_mutation(false);
}

fn relational_mutation(seed_match: bool) {
    // A prior match keeps common-ancestor marks stable; the first match changes
    // them too. Both must match fresh, but only the former proves a clean skip.
    let seed = if seed_match {
        "<section><i class=hot></i></section>"
    } else {
        ""
    };
    let (input, values, mut cache, _) = build(&format!(
        "<!doctype html><style>section:has(.hot){{color:red}}</style><main id=m>{seed}<section id=p><i id=c class=cold></i></section><aside id=q><b></b></aside></main>",
    ));
    let c = handle(&input, "c");
    let p = handle(&input, "p").key();
    let q = handle(&input, "q").key();
    let old_p = cache.raw().get(p).unwrap().clone();
    let old_q = cache.raw().get(q).unwrap().clone();
    let NodeData::Element { attrs, .. } = &c.data else {
        panic!("element")
    };
    attrs
        .borrow_mut()
        .iter_mut()
        .find(|a| &*a.name.local == "class")
        .unwrap()
        .value = "hot".into();
    let probe = cache
        .probe(&input, "", Media::default(), &values)
        .unwrap()
        .unwrap();
    cache.targets(
        &probe,
        &crate::dom::invalidation::Changed {
            classes: vec!["cold".into(), "hot".into()],
            attrs: vec!["class".into()],
            ..Default::default()
        },
        std::slice::from_ref(&c),
    );
    let mut bindings = Bindings::default();
    run(
        &input,
        &values,
        &mut cache,
        &change(
            c.key(),
            SourceChangeKind::Attributes(Vec::new()),
            Some(p),
            Some(p),
        ),
        &mut bindings,
    )
    .unwrap();
    if seed_match {
        assert!(bindings.reused.contains(&q));
        assert!(Rc::ptr_eq(cache.raw().get(q).unwrap(), &old_q));
    }
    let mut compared = CascadedMemo::default();
    let (Node::Element(before), Node::Element(after)) =
        (&old_p.raw[0], &cache.raw().get(p).unwrap().raw[0])
    else {
        panic!("element")
    };
    assert!(!crate::dom::same_cascaded_tree(
        before,
        after,
        false,
        &mut compared
    ));
    assert_fresh(&input, &cache);
}

#[test]
fn retained_cascade_uses_the_prepared_page_scheme_for_new_declarations() {
    let (input, values, mut cache, _) = build(
        "<!doctype html><meta name=color-scheme content=dark><div id=p style='color:red'></div><aside id=q></aside>",
    );
    let p = handle(&input, "p");
    let q = handle(&input, "q").key();
    let old_q = cache.raw().get(q).unwrap().clone();
    set_style(&p, "color:light-dark(red,green)");
    let mut bindings = Bindings::default();
    run(
        &input,
        &values,
        &mut cache,
        &change(
            p.key(),
            SourceChangeKind::Attributes(Vec::new()),
            None,
            None,
        ),
        &mut bindings,
    )
    .unwrap();
    assert!(bindings.reused.contains(&q));
    assert!(Rc::ptr_eq(cache.raw().get(q).unwrap(), &old_q));
    assert_fresh(&input, &cache);
}
