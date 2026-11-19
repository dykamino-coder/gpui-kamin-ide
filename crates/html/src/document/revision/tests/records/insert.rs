use super::*;

fn place(parent: &crate::SourceNode, child: &crate::SourceNode, before: Option<&Handle>) -> Mutation {
    Mutation::InsertBefore {
        parent: parent.clone(),
        child: child.clone(),
        before: before.cloned(),
    }
}

#[test]
fn insert_before_and_append_move_the_same_nodes_in_record_order() {
    let mut doc = Document::builder("<main><i>a</i><b>b</b></main>", "")
        .build()
        .unwrap();
    let main = doc.source_node(&element(&doc, "main")).unwrap();
    let i = doc.source_node(&element(&doc, "i")).unwrap();
    let b = doc.source_node(&element(&doc, "b")).unwrap();
    let em = doc
        .create_element(html5ever::QualName::new(
            None,
            html5ever::ns!(html),
            "em".into(),
        ))
        .unwrap();
    let c = doc.create_text_node("c").unwrap();
    doc.apply(&[place(&em, &c, None)], "").unwrap();
    doc.apply(
        &[
            place(&main, &em, Some(b.handle())),
            place(&b, &i, None),
        ],
        "",
    )
    .unwrap();
    let fresh = Document::builder("<main><em>c</em><b>b<i>a</i></b></main>", "")
        .build()
        .unwrap();
    assert_eq!(state(&doc), state(&fresh));
    assert_eq!(tree(&doc), tree(&fresh));
    assert!(Rc::ptr_eq(i.handle(), &element(&doc, "i")));
    doc.apply(
        &[Mutation::RemoveChild {
            parent: main.handle().clone(),
            child: em.handle().clone(),
        }],
        "",
    )
    .unwrap();
    doc.apply(&[place(&main, &em, None)], "").unwrap();
    assert!(Rc::ptr_eq(&main.handle().children.borrow()[1], em.handle()));
}

#[test]
fn cycles_foreign_owners_and_stale_reference_nodes_refuse_the_whole_batch() {
    let html = "<main><i>a</i><b>b</b></main>";
    let mut doc = Document::builder(html, "").build().unwrap();
    let other = Document::builder(html, "").build().unwrap();
    let main = doc.source_node(&element(&doc, "main")).unwrap();
    let i = doc.source_node(&element(&doc, "i")).unwrap();
    let b = doc.source_node(&element(&doc, "b")).unwrap();
    let foreign = other.source_node(&element(&other, "i")).unwrap();
    let root = doc.source_node(&doc.source_dom()).unwrap();
    let salt = doc.salt();
    for (bad, expected) in [
        (place(&b, &main, None), "hierarchy"),
        (place(&main, &foreign, None), "foreign"),
        (place(&foreign, &i, None), "foreign"),
        (place(&main, &b, Some(i.handle())), "reference"),
        (place(&root, &b, None), "hierarchy"),
    ] {
        let error = doc
            .apply(&[place(&b, &i, None), bad], "")
            .unwrap_err();
        assert!(matches!(
            (error, expected),
            (ApplyError::Hierarchy, "hierarchy")
                | (ApplyError::Foreign, "foreign")
                | (ApplyError::NotChild, "reference")
        ));
        assert_eq!(doc.salt(), salt);
        assert!(Rc::ptr_eq(&main.handle().children.borrow()[0], i.handle()));
        assert!(Rc::ptr_eq(&main.handle().children.borrow()[1], b.handle()));
    }
    assert!(
        !doc.apply(
            &[place(&main, &i, Some(i.handle())), place(&main, &b, None)],
            ""
        )
        .unwrap()
        .changed()
    );
}

#[test]
fn refusing_a_move_restores_both_parents_and_the_detached_allocation() {
    use crate::budget::{BudgetKind, DocumentLimits};
    let mut doc = Document::builder("<main><i>a</i><b>b</b></main>", "")
        .limits(
            DocumentLimits::default()
                .lower(BudgetKind::SubstitutionBytes, 1)
                .unwrap(),
        )
        .build()
        .unwrap();
    let main = doc.source_node(&element(&doc, "main")).unwrap();
    let i = doc.source_node(&element(&doc, "i")).unwrap();
    let b = doc.source_node(&element(&doc, "b")).unwrap();
    let new = doc.create_text_node("new").unwrap();
    let records = [
        place(&b, &i, None),
        place(&main, &new, Some(b.handle())),
        attr(main.handle(), "style", Some("--a: green; color: var(--a)")),
    ];
    let salt = doc.salt();
    assert!(matches!(
        doc.apply(&records, ""),
        Err(ApplyError::Rejected(_))
    ));
    assert_eq!(doc.salt(), salt);
    assert!(new.handle().parent.take().is_none());
    assert!(Rc::ptr_eq(&main.handle().children.borrow()[0], i.handle()));
    assert_eq!(b.handle().children.borrow().len(), 1);
    assert!(doc.apply(&records[..2], "").unwrap().changed());
    assert!(Rc::ptr_eq(&main.handle().children.borrow()[0], new.handle()));
    assert!(Rc::ptr_eq(&b.handle().children.borrow()[1], i.handle()));
}
