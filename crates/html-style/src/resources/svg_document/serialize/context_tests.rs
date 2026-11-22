//! Тесты: сериализация изолирует состояние документа и восстанавливает окружающий viewport.

use super::{serialize, serialize_sized, write_element};
use crate::dom::{Element, Node, parse};
use crate::style::values::value::Len;
use crate::values::DocumentValues;

fn find(nodes: &[Node], tag: &str) -> Option<Element> {
    nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == tag => Some(e.clone()),
        Node::Element(e) => find(&e.children, tag),
        Node::Text(_) => None,
    })
}

fn translate_rects(e: &mut Element) {
    if e.tag == "rect" {
        e.style.used_mut().reset_transforms.transform = Some(Default::default());
        e.style.used_mut().reset_transforms.translate = Some((Len::Pct(0.5), Len::Pct(0.5)));
    }
    for child in &mut e.children {
        if let Node::Element(child) = child {
            translate_rects(child);
        }
    }
}

#[test]
fn sized_document_does_not_change_independent_serialization() {
    let doc = DocumentValues::detached(Default::default());
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
    let doc = DocumentValues::detached(Default::default());
    let mut root = find(
        &parse(r#"<svg width="200" height="100"><rect/><svg width="80" height="40"><rect/></svg><rect/></svg>"#, ""),
        "svg",
    ).expect("svg");
    translate_rects(&mut root);
    let out = serialize(&root, doc.values());
    assert_eq!(out.matches("translate(100 50)").count(), 2, "{out}");
    assert_eq!(out.matches("translate(40 20)").count(), 1, "{out}");
}

#[test]
fn used_viewport_preserves_the_natural_size_and_expands_viewbox_from_css_size() {
    let doc = DocumentValues::detached(Default::default());
    let root = find(
        &parse(
            r#"<svg width="162" height="81" viewBox="0 0 162 81"><rect/></svg>"#,
            "",
        ),
        "svg",
    )
    .expect("svg");
    let natural = super::size_of(&root);
    let out = super::serialize_viewport(&root, (90.0, 45.0), 100.0, 50.0, 0.0, 0.0, doc.values());
    assert!(out.contains(r#"viewBox="0 0 180 90""#), "{out}");
    assert_eq!(super::size_of(&root), natural);
}

#[test]
fn used_viewport_supplies_the_child_percentage_reference_without_a_viewbox() {
    let doc = DocumentValues::detached(Default::default());
    let mut root = find(
        &parse(r#"<svg width="162" height="81"><rect/></svg>"#, ""),
        "svg",
    )
    .expect("svg");
    translate_rects(&mut root);
    let out = super::serialize_viewport(&root, (90.0, 45.0), 90.0, 45.0, 0.0, 0.0, doc.values());
    assert!(out.contains("translate(45 22.5)"), "{out}");
    assert_eq!(super::size_of(&root), (162.0, 81.0));
}
