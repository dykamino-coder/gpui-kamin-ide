//! Child removals through the same source DOM as scalar mutation records.
use super::*;

fn remove(parent: &Handle, child: &Handle) -> Mutation {
    Mutation::RemoveChild {
        parent: parent.clone(),
        child: child.clone(),
    }
}

#[test]
fn removing_children_keeps_survivor_keys_and_matches_fresh() {
    let css = "<style>section:empty{background:blue} main>p:first-child{color:green}</style>";
    let mut doc = Document::builder(
        &format!("{css}<main><i>A</i><p>B</p><section>C</section><b>D</b></main>"),
        "",
    )
    .build()
    .unwrap();
    let main = element(&doc, "main");
    let p = element(&doc, "p");
    let key = p.key();
    let i = element(&doc, "i");
    let b = element(&doc, "b");
    let section = element(&doc, "section");
    let c = text(&doc, "C");
    let content = text(&doc, "B");
    let records = [
        remove(&main, &b),
        attr(&p, "class", Some("old")),
        data(&content, "intermediate"),
        attr(&section, "title", Some("changed")),
        data(&c, "removed text"),
        remove(&section, &c),
        data(&content, "new"),
        attr(&p, "class", Some("final")),
        remove(&main, &i),
    ];
    let dom = doc.source_dom();
    assert!(doc.apply(&records, "").unwrap().changed());
    assert!(Rc::ptr_eq(&dom, &doc.source_dom()));
    assert!(Rc::ptr_eq(&p, &element(&doc, "p")));
    assert_eq!(p.key(), key);
    for removed in [i, b, c] {
        assert!(removed.parent.take().is_none());
    }
    let fresh = Document::builder(
        &format!("{css}<main><p class=final>new</p><section title=changed></section></main>"),
        "",
    )
    .build()
    .unwrap();
    assert_eq!(tree(&doc), tree(&fresh));
    assert_eq!(state(&doc), state(&fresh));
}

#[test]
fn an_invalid_child_record_leaves_earlier_records_unapplied() {
    let mut doc = Document::builder("<main><i>a</i><b>b</b></main>", "")
        .build()
        .unwrap();
    let main = element(&doc, "main");
    let i = element(&doc, "i");
    let b = element(&doc, "b");
    let a = text(&doc, "a");
    let salt = doc.salt();
    for bad in [remove(&main, &i), remove(&b, &a)] {
        let records = [data(&a, "changed"), remove(&main, &i), bad];
        assert!(doc.apply(&records, "").is_err());
        assert_eq!(doc.salt(), salt);
        assert!(Rc::ptr_eq(&main.children.borrow()[0], &i));
        assert!(Rc::ptr_eq(&text_parent(&doc, "a"), &i));
    }
}

#[test]
fn a_refused_child_edit_restores_sibling_order_parent_links_and_scalar_values() {
    use crate::budget::{BudgetKind, DocumentLimits, ResourceGovernor};
    let mut doc = Document::builder(
        "<main><i>a</i><p>b</p><b>c</b><u>d</u><em>e</em></main>",
        "",
    )
    .limits(
        DocumentLimits::default()
            .lower(BudgetKind::SubstitutionBytes, 1)
            .unwrap(),
    )
    .governor(&ResourceGovernor::default())
    .build()
    .unwrap();
    let main = element(&doc, "main");
    let before = main.children.borrow().clone();
    let records = [
        remove(&main, &before[4]),
        remove(&main, &before[0]),
        remove(&main, &before[2]),
        attr(&main, "style", Some("--a: green; color: var(--a)")),
    ];
    let salt = doc.salt();
    assert!(matches!(
        doc.apply(&records, ""),
        Err(ApplyError::Rejected(_))
    ));
    assert_eq!(doc.salt(), salt);
    assert_eq!(main.children.borrow().len(), before.len());
    for (i, node) in before.iter().enumerate() {
        assert!(Rc::ptr_eq(node, &main.children.borrow()[i]));
        let link = node.parent.take().unwrap();
        assert!(Rc::ptr_eq(&link.upgrade().unwrap(), &main));
        node.parent.set(Some(link));
    }
    assert!(doc.apply(&records[..3], "").unwrap().changed());
    assert_eq!(main.children.borrow().len(), 2);
}

#[test]
fn records_can_edit_a_subtree_removed_earlier_in_the_same_batch() {
    let mut doc = Document::builder("<main><i>a</i></main>", "")
        .build()
        .unwrap();
    let main = element(&doc, "main");
    let i = element(&doc, "i");
    let a = text(&doc, "a");
    let records = [remove(&main, &i), data(&a, "updated"), remove(&i, &a)];
    doc.apply(&records, "").unwrap();
    assert!(main.children.borrow().is_empty());
    assert!(i.children.borrow().is_empty());
    assert!(i.parent.take().is_none() && a.parent.take().is_none());
    assert!(matches!(&a.data, NodeData::Text {contents} if &**contents.borrow()=="updated"));
}
#[test]
fn a_valid_child_pair_from_another_document_cannot_remove_a_local_child() {
    let html = "<main><i>a</i></main>";
    let mut doc = Document::builder(html, "").build().unwrap();
    let other = Document::builder(html, "").build().unwrap();
    let here = element(&doc, "i");
    let there = element(&other, "i");
    assert_eq!(here.key(), there.key());
    let parent = element(&other, "main");
    let records = [data(&text(&doc, "a"), "b"), remove(&parent, &there)];
    let salt = doc.salt();
    assert!(matches!(doc.apply(&records, ""), Err(ApplyError::Foreign)));
    assert_eq!(doc.salt(), salt);
    assert!(Rc::ptr_eq(&text_parent(&doc, "a"), &here));
    assert!(Rc::ptr_eq(&parent.children.borrow()[0], &there));
}
