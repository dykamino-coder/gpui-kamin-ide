use super::*;

fn place(parent: &Handle, child: &crate::SourceNode, before: Option<&Handle>) -> Mutation {
    Mutation::InsertBefore { parent: parent.clone(), child: child.clone(), before: before.cloned() }
}

#[test]
fn insert_before_and_append_move_the_same_nodes_in_record_order() {
    let mut doc = Document::builder("<main><i>a</i><b>b</b></main>", "").build().unwrap();
    let main = element(&doc, "main");
    let i = doc.source_node(&element(&doc, "i")).unwrap();
    let b = element(&doc, "b");
    let em = doc.create_element(html5ever::QualName::new(None, html5ever::ns!(html), "em".into())).unwrap();
    let c = doc.create_text_node("c").unwrap();
    doc.apply(&[place(em.handle(), &c, None), place(&main, &em, Some(&b)), place(&b, &i, None)], "").unwrap();
    let fresh = Document::builder("<main><em>c</em><b>b<i>a</i></b></main>", "").build().unwrap();
    assert_eq!(state(&doc), state(&fresh));
    assert_eq!(tree(&doc), tree(&fresh));
    assert!(Rc::ptr_eq(i.handle(), &element(&doc, "i")));
    doc.apply(&[Mutation::RemoveChild {parent: main.clone(), child: em.handle().clone()}], "").unwrap();
    doc.apply(&[place(&main, &em, None)], "").unwrap();
    assert!(Rc::ptr_eq(&main.children.borrow()[1], em.handle()));
}
