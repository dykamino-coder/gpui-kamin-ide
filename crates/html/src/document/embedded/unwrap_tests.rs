//! Снятая обёртка `<html>` передаёт свой `filter` содержимому.

use super::*;
use crate::dom::Element;

fn find<'a>(nodes: &'a [Node], tag: &str) -> &'a Element {
    crate::dom::test_support::find(nodes, &|e| e.tag == tag).unwrap()
}

fn unwrapped(body: &str) -> Vec<Node> {
    let parsed = crate::dom::parse(
        &format!(r#"<!DOCTYPE html><html style="filter: opacity(0.5)">{body}</html>"#),
        "",
    );
    let (nodes, _) = unwrap_document(parsed.nodes.clone(), parsed.values());
    assert!(crate::dom::test_support::find(&nodes, &|e| e.tag == "html").is_none());
    nodes
}

fn filter(e: &Element) -> Option<(f32, f32)> {
    e.style.cascaded().filter.map(|f| (f.opacity, f.brightness))
}

#[test]
fn unwrapped_root_filter_moves_to_content() {
    let nodes = unwrapped("<body><p>a</p></body>");
    let p = find(&nodes, "p");
    assert_eq!(filter(p), Some((0.5, 1.0)));
    assert_eq!(p.style.cascaded().scrollbar_filter_ops.len(), 1);
}

#[test]
fn own_filter_of_unwrapped_content_wins() {
    let nodes = unwrapped(r#"<body style="filter: brightness(50%)"><p>a</p></body>"#);
    assert_eq!(filter(find(&nodes, "p")), Some((1.0, 0.5)));
}
