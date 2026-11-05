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
fn twenty_thousand_svg_levels_serialize_filters_and_ids_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let doc = DocumentValues::detached(Default::default());
            let mut root = find(&parse("<svg><g/></svg>", ""), "g").expect("g");
            let mut flood = root.clone();
            flood.tag = "feFlood".into();
            flood.attrs = vec![
                ("flood-color".into(), "currentColor".into()),
                ("result".into(), "map".into()),
            ];
            let mut map = root.clone();
            map.tag = "feDisplacementMap".into();
            map.attrs = vec![("in2".into(), "map".into())];
            let mut filter = root.clone();
            filter.tag = "filter".into();
            filter.children = vec![Node::Element(flood), Node::Element(map)].into();
            root.children = vec![Node::Text("kamin-clip & <".into()), Node::Element(filter)].into();
            for _ in 0..20_000 {
                let mut parent = root.clone();
                parent.children = vec![Node::Element(root)].into();
                root = parent;
            }
            let out = serialize(&root, doc.values());
            assert_eq!(out.matches("</g>").count(), 20_001);
            assert!(out.contains("kamin-clip &amp; &lt;"));
            assert!(out.contains("<feOffset"));
            assert!(!out.contains("<feDisplacementMap"));
        })
        .expect("spawn")
        .join()
        .expect("deep SVG");
}
