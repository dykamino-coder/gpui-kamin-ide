//! Тесты: сериализация изолирует состояние документа и восстанавливает окружающий viewport.

use super::{serialize, serialize_sized, write_element};
use crate::document::state::DocState;
use crate::dom::{Element, Node, parse};
use crate::style::values::value::Len;

fn find(nodes: &[Node], tag: &str) -> Option<Element> {
    nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == tag => Some(e.clone()),
        Node::Element(e) => find(&e.children, tag),
        Node::Text(_) => None,
    })
}

fn translate_rects(e: &mut Element) {
    if e.tag == "rect" {
        e.style.transform = Some(Default::default());
        e.style.translate = Some((Len::Pct(0.5), Len::Pct(0.5)));
    }
    for child in &mut e.children {
        if let Node::Element(child) = child {
            translate_rects(child);
        }
    }
}

#[test]
fn sized_document_does_not_change_independent_serialization() {
    let doc = DocState::new(Default::default());
    let large = find(&parse(r#"<svg width="800" height="600"/>"#, ""), "svg").expect("svg");
    let mut rect = find(&parse("<svg><rect/></svg>", ""), "rect").expect("rect");
    translate_rects(&mut rect);
    let mut before = String::new();
    write_element(&rect, &mut before, doc.values());
    serialize_sized(&large, 800.0, 600.0, 0.0, 0.0, doc.values());
    let mut after = String::new();
    write_element(&rect, &mut after, doc.values());
    assert_eq!(after, before);
    assert!(after.contains("translate(150 75)"), "{after}");
}

#[test]
fn nested_viewport_does_not_change_following_sibling() {
    let doc = DocState::new(Default::default());
    let mut root = find(
        &parse(r#"<svg width="200" height="100"><rect/><svg width="80" height="40"><rect/></svg><rect/></svg>"#, ""),
        "svg",
    ).expect("svg");
    translate_rects(&mut root);
    let out = serialize(&root, doc.values());
    assert_eq!(out.matches("translate(100 50)").count(), 2, "{out}");
    assert_eq!(out.matches("translate(40 20)").count(), 1, "{out}");
}
