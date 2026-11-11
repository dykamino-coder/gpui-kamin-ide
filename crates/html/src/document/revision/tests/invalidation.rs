//! The names a batch of attribute mutations changes (`Document::invalidation`)
//! and the elements a rebuild for it restyles (`dom::invalidation`).

use super::super::mutation::{Batch, Mutation};
use super::PAGE;
use crate::document::Document;
use crate::dom::invalidation::Changed;
use markup5ever_rcdom::{Handle, NodeData};

fn element(node: &Handle, local: &str) -> Option<Handle> {
    if matches!(&node.data, NodeData::Element { name, .. } if &*name.local == local) {
        return Some(node.clone());
    }
    let children = node.children.borrow();
    children.iter().find_map(|c| element(c, local))
}

fn set(element: &Handle, name: &str, value: &str) -> Mutation {
    Mutation::Attribute {
        element: element.clone(),
        name: html5ever::QualName::new(None, html5ever::ns!(), name.into()),
        value: Some(value.into()),
    }
}

fn changed(doc: &Document, records: &[Mutation]) -> Option<Changed> {
    doc.invalidation(&Batch::new(records).unwrap(), "")
}

#[test]
fn a_batch_names_its_classes_and_data_attributes_and_their_elements() {
    let doc = Document::builder(PAGE, "").build().unwrap();
    let (html, p) = (
        element(&doc.source_dom(), "html").unwrap(),
        element(&doc.source_dom(), "p").unwrap(),
    );
    let found = changed(
        &doc,
        &[set(&html, "class", "alt on"), set(&p, "data-k", "1")],
    )
    .unwrap();
    assert_eq!(found.classes, ["alt", "on"]);
    assert_eq!(found.attrs, ["class", "data-k"]);
    let ids: Vec<u64> = found.elements;
    assert_eq!(ids.len(), 2);
    // The walk leaves `html` last: its id is the count of elements.
    assert_eq!(ids[0], doc.state.values.traversal.elements().len() as u64);
    for other in ["id", "style", "lang", "data-kamin-x"] {
        assert_eq!(changed(&doc, &[set(&p, other, "x")]), None, "{other}");
    }
    let text = p.children.borrow()[0].clone();
    let typed = Mutation::CharacterData {
        node: text,
        data: "z".into(),
    };
    assert_eq!(changed(&doc, &[typed]), None, "texts");
    assert_eq!(
        doc.invalidation(&Batch::new(&[set(&p, "class", "on")]).unwrap(), "p{}"),
        None,
        "theme"
    );
}

#[test]
fn a_rebuild_restyles_the_elements_of_the_rules_that_read_a_changed_class() {
    let page = PAGE.replace(".on{", "html.alt body{color:red} .on{");
    let doc = Document::builder(&page, "").build().unwrap();
    let html = element(&doc.source_dom(), "html").unwrap();
    let changed = changed(&doc, &[set(&html, "class", "alt")]).unwrap();
    let state = crate::document::state::DocState::continuing(doc.state.media(), &doc.state);
    let mut invalidation = crate::dom::invalidation::Invalidation::new(changed.clone());
    let fonts = doc.state.text.metrics.font_context();
    let html_text = page.replace("<html>", "<html class=\"alt\">");
    let revision = doc.budget().revision();
    let source = Document::admit(&html_text, "", None, false, &revision).unwrap();
    let next = Document::try_source(
        source,
        "",
        None,
        false,
        state,
        &fonts,
        revision,
        Some(&mut invalidation),
    );
    assert!(next.is_ok());
    let restyled = invalidation.finish().expect("known");
    // `body` is left just before `html`.
    assert_eq!(restyled, [changed.elements[0], changed.elements[0] - 1]);
}
