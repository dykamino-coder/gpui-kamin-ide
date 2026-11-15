//! Inline boundaries preserve word context and descendant text inputs invalidate their owner.

use super::*;
use crate::document::Document;

fn paragraph(doc: &Document) -> &crate::dom::Element {
    let mut pending = vec![doc.nodes.iter()];
    while let Some(siblings) = pending.last_mut() {
        match siblings.next() {
            Some(Node::Element(e)) if e.tag == "p" => return e,
            Some(Node::Element(e)) => pending.push(e.children.iter()),
            Some(Node::Text(_)) => {}
            None => {
                pending.pop();
            }
        }
    }
    panic!("paragraph");
}

fn content(doc: &Document) -> (BoxId, Rc<InlineContent>) {
    crate::style::cascade::materialize::materialize(
        &doc.nodes,
        &Rc::new(Computed::default()),
        doc.values(),
    );
    doc.state
        .boxes
        .borrow_mut()
        .finish(&doc.nodes, &BoxStyle::default(), true);
    let p = paragraph(doc);
    let tree = doc.state.boxes.borrow();
    let id = tree.flows[&slice_key(&p.children)];
    (id, tree.normalized_inline(id, &p.style.used()).unwrap())
}

#[test]
fn mixed_text_preserves_cross_box_word_context_and_utf8_boundaries() {
    let doc = Document::builder(
        "<p style='text-transform:capitalize'>hel<b>lo</b> <i>école</i> A<span>V</span> سلام</p>",
        "",
    )
    .build()
    .unwrap();
    // Descendants must not use a cascaded value as their inherited computed input.
    {
        let p = paragraph(&doc);
        let tree = doc.state.boxes.borrow();
        let id = tree.flows[&slice_key(&p.children)];
        assert!(tree.normalized_inline(id, &p.style.used()).is_none());
    }
    let (_, text) = content(&doc);
    assert_eq!(text.text, "Hello École AV سلام");
    assert_eq!(
        text.items
            .iter()
            .filter(|i| i.kind == ItemKind::Open)
            .count(),
        3
    );
    assert_eq!(
        text.items
            .iter()
            .filter(|i| i.kind == ItemKind::Close)
            .count(),
        3
    );
    for item in &text.items {
        assert!(doc.state.boxes.borrow().get(item.box_id).is_some());
        if let ItemKind::Text { start, end } = item.kind {
            assert!(text.text.is_char_boundary(start) && text.text.is_char_boundary(end));
        }
    }
}

#[test]
fn only_read_descendant_inputs_replace_the_normalized_owner() {
    let html = "<p>before <b>mixed</b> after</p>";
    let mut doc = Document::builder(html, "b {text-transform:uppercase}")
        .build()
        .unwrap();
    let (id, old) = content(&doc);
    assert_eq!(old.text, "before MIXED after");
    doc.update(
        html,
        "b {text-transform:uppercase;color:red;font-size:28px}",
    )
    .unwrap();
    {
        let p = paragraph(&doc);
        assert!(
            doc.state
                .boxes
                .borrow()
                .normalized_inline(id, &p.style.used())
                .is_none()
        );
    }
    let (same_id, painted) = content(&doc);
    assert_eq!(same_id, id);
    assert!(Rc::ptr_eq(&old, &painted));
    doc.update(
        html,
        "b {text-transform:lowercase;color:red;font-size:28px}",
    )
    .unwrap();
    let (same_id, changed) = content(&doc);
    assert_eq!(same_id, id);
    assert_eq!(changed.text, "before mixed after");
    assert_eq!(old.text, "before MIXED after");
    assert!(!Rc::ptr_eq(&old, &changed));
}
