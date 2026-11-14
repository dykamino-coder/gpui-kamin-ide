//! IFC сохраняет логический порядок, владельца текста и старые immutable входы.

use super::*;
use crate::document::Document;
use crate::dom::Node;

fn flow(doc: &Document, tag: &str) -> Rc<InlineContent> {
    let mut pending: Vec<_> = doc.nodes.iter().collect();
    let source = loop {
        let Node::Element(element) = pending.pop().expect("flow element") else {
            continue;
        };
        if element.tag == tag {
            break element.node_id;
        }
        pending.extend(element.children.iter());
    };
    let tree = doc.state.boxes.borrow();
    tree.arena
        .iter()
        .find_map(|(_, record)| {
            (record.source == source)
                .then(|| record.inline.clone())
                .flatten()
        })
        .expect("logical flow")
}

#[test]
fn utf8_runs_keep_inline_boundaries_and_atoms_without_duplicate_text() {
    let doc = Document::builder("<!doctype html><p>а<b>漢</b><img src=x><br>z</p>", "")
        .build()
        .unwrap();
    let content = flow(&doc, "p");
    assert_eq!(content.text, "а漢z");
    let ranges: Vec<_> = content
        .items
        .iter()
        .filter_map(|item| {
            if let ItemKind::Text(range) = &item.kind {
                Some(&content.text[range.clone()])
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ranges, ["а", "漢", "z"]);
    assert_eq!(
        content
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Open)
            .count(),
        1
    );
    assert_eq!(
        content
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Close)
            .count(),
        1
    );
    assert_eq!(
        content
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Atom)
            .count(),
        1
    );
    assert_eq!(
        content
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::HardBreak)
            .count(),
        1
    );
    let tree = doc.state.boxes.borrow();
    assert!(
        content
            .items
            .iter()
            .all(|item| tree.get(item.box_id).is_some())
    );
    let owned: Vec<_> = tree
        .arena
        .iter()
        .filter_map(|(_, record)| record.inline.as_ref())
        .filter(|flow| !flow.text.is_empty())
        .collect();
    assert_eq!(
        owned.len(),
        1,
        "inline ancestors do not retain the same text"
    );
}

#[test]
fn atom_and_out_of_flow_text_have_their_own_owners() {
    let doc = Document::builder(
        "<main>before<span style='display:inline-block'>atom</span><i style='position:absolute'>overlay</i>after</main>", ""
    ).build().unwrap();
    let content = flow(&doc, "main");
    assert_eq!(content.text, "beforeafter");
    assert!(content.items.iter().any(|item| item.kind == ItemKind::Atom));
    assert!(
        content
            .items
            .iter()
            .any(|item| item.kind == ItemKind::OutOfFlow)
    );
    assert_eq!(flow(&doc, "span").text, "atom");
    assert_eq!(flow(&doc, "i").text, "overlay");
}

#[test]
fn commit_preserves_unchanged_flow_and_old_consumer_keeps_its_text() {
    let before = "<p>old <b>text</b></p>";
    let mut doc = Document::builder(before, "").build().unwrap();
    let old = flow(&doc, "p");
    doc.update("<p class=unused>old <b>text</b></p>", "")
        .unwrap();
    assert!(Rc::ptr_eq(&old, &flow(&doc, "p")));
    doc.update("<p class=unused>new <b>text</b></p>", "")
        .unwrap();
    let new = flow(&doc, "p");
    assert_eq!(old.text, "old text");
    assert_eq!(new.text, "new text");
    assert!(!Rc::ptr_eq(&old, &new));
}
