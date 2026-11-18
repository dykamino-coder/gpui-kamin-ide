//! An IFC keeps the logical order, the owner of the text and old immutable inputs.

use super::*;
use crate::document::Document;
use crate::dom::Node;

pub(super) fn flow(doc: &Document, tag: &str) -> InlineContent {
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
    let doc = Document::builder("<!doctype html><p>é<b>漢</b><img src=x><br>z</p>", "")
        .build()
        .unwrap();
    let content = flow(&doc, "p");
    assert_eq!(content.text(), "é漢z");
    let ranges: Vec<_> = content
        .items()
        .iter()
        .filter_map(|item| {
            if let ItemKind::Text { start, end } = item.kind {
                Some(&content.text()[start as usize..end as usize])
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ranges, ["é", "漢", "z"]);
    assert_eq!(
        content
            .items()
            .iter()
            .filter(|item| item.kind == ItemKind::Open)
            .count(),
        1
    );
    assert_eq!(
        content
            .items()
            .iter()
            .filter(|item| item.kind == ItemKind::Close)
            .count(),
        1
    );
    assert_eq!(
        content
            .items()
            .iter()
            .filter(|item| item.kind == ItemKind::Atom)
            .count(),
        1
    );
    assert_eq!(
        content
            .items()
            .iter()
            .filter(|item| item.kind == ItemKind::HardBreak)
            .count(),
        1
    );
    let tree = doc.state.boxes.borrow();
    assert!(
        content
            .items()
            .iter()
            .all(|item| tree.get(item.box_id).is_some())
    );
    let owned: Vec<_> = tree
        .arena
        .iter()
        .filter_map(|(_, record)| record.inline.as_ref())
        .filter(|flow| !flow.text().is_empty())
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
    assert_eq!(content.text(), "beforeafter");
    assert!(
        content
            .items()
            .iter()
            .any(|item| item.kind == ItemKind::Atom)
    );
    assert!(
        content
            .items()
            .iter()
            .any(|item| item.kind == ItemKind::OutOfFlow)
    );
    assert_eq!(flow(&doc, "span").text(), "atom");
    assert_eq!(flow(&doc, "i").text(), "overlay");
}

#[test]
fn commit_preserves_unchanged_flow_and_old_consumer_keeps_its_text() {
    let before = "<p>old <b>text</b></p>";
    let mut doc = Document::builder(before, "").build().unwrap();
    let old = flow(&doc, "p");
    doc.update("<p class=unused>old <b>text</b></p>", "")
        .unwrap();
    assert!(old.same(&flow(&doc, "p")));
    doc.update("<p class=unused>new <b>text</b></p>", "")
        .unwrap();
    let new = flow(&doc, "p");
    assert_eq!(old.text(), "old text");
    assert_eq!(new.text(), "new text");
    assert!(!old.same(&new));
}

#[test]
fn contents_and_break_opportunities_keep_the_enclosing_flow() {
    let doc = Document::builder(
        "<main>before<span style='display:contents'>inside</span><wbr><br style='display:contents'>after</main>",
        "",
    ).build().unwrap();
    let content = flow(&doc, "main");
    assert_eq!(content.text(), "beforeinsideafter");
    assert_eq!(
        content
            .items()
            .iter()
            .filter(|i| i.kind == ItemKind::Opportunity)
            .count(),
        1,
    );
    assert!(!content.items().iter().any(|i| matches!(
        i.kind,
        ItemKind::Open | ItemKind::Close | ItemKind::Block | ItemKind::HardBreak
    )));
}

#[test]
fn ruby_is_a_leaf_in_the_parent_flow() {
    let doc = Document::builder(
        "<main>before<ruby>base<rt>annotation</rt></ruby>after</main>",
        "",
    )
    .build()
    .unwrap();
    let content = flow(&doc, "main");
    assert_eq!(content.text(), "beforeafter");
    assert!(content.items().iter().any(|i| i.kind == ItemKind::Atom));
    assert!(!flow(&doc, "ruby").text().contains("annotation"));
    assert!(
        flow(&doc, "ruby")
            .items()
            .iter()
            .any(|i| i.kind == ItemKind::RubyAnnotation)
    );
    assert_eq!(flow(&doc, "rt").text(), "annotation");
}

#[test]
fn a_root_inline_fragment_retains_its_text() {
    let mut element =
        crate::layout::table::anon::anon_element("span", vec![Node::Text("fragment".into())]);
    element.inline = true;
    let mut tree = super::super::BoxTree::default();
    tree.publish(&[Node::Element(element)]);
    assert_eq!(
        tree.get(tree.first)
            .unwrap()
            .inline
            .as_ref()
            .unwrap()
            .text(),
        "fragment"
    );
}

#[test]
fn root_text_and_inline_siblings_share_one_document_flow() {
    let mut span =
        crate::layout::table::anon::anon_element("span", vec![Node::Text("middle".into())]);
    span.inline = true;
    let mut tree = super::super::BoxTree::default();
    tree.publish(&[
        Node::Text("before".into()),
        Node::Element(span),
        Node::Text("after".into()),
    ]);
    let root = tree.first;
    let content = tree.get(root).unwrap().inline.as_ref().unwrap();
    assert_eq!(content.text(), "beforemiddleafter");
    tree.publish(&[Node::Text("changed".into())]);
    assert_eq!(tree.first, root);
    assert_eq!(
        tree.get(root).unwrap().inline.as_ref().unwrap().text(),
        "changed"
    );
}

#[test]
fn root_contents_siblings_share_the_document_flow() {
    let contents = |text: &str| {
        let mut element =
            crate::layout::table::anon::anon_element("div", vec![Node::Text(text.into())]);
        element.style.cascaded_mut().reset_box_layout.display = Some(Display::Contents);
        Node::Element(element)
    };
    let mut tree = super::super::BoxTree::default();
    tree.publish(&[contents("left"), contents("right")]);
    assert_eq!(
        tree.get(tree.first)
            .unwrap()
            .inline
            .as_ref()
            .unwrap()
            .text(),
        "leftright"
    );
}
