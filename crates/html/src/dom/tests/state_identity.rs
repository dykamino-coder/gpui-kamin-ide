//! Идентичность соседних синтетических scroll-коробок в одном scope GPUI.

use crate::dom::{Node, parse};

#[test]
fn generated_scroll_boxes_have_distinct_stable_state_ids() {
    fn generated(nodes: &[Node], out: &mut Vec<(String, gpui::ElementId)>) {
        for node in nodes {
            if let Node::Element(element) = node {
                if element.tag.starts_with("::") {
                    assert!(element.style.overflow_y.is_some());
                    out.push((element.tag.clone(), element.state_id("html-scroll-area")));
                }
                generated(&element.children, out);
            }
        }
    }
    let html = r#"<style>
        div::before, div::after { content: 'scroll' }
        div::before, div::after, details::details-content {
            display: block; overflow: scroll;
            width: 20px; height: 10px;
        }
    </style><div></div><div></div>
    <details open>first</details><details open>second</details>"#;
    let parsed = parse(html, "");
    let nodes = &parsed.nodes;
    let mut ids = Vec::new();
    generated(nodes, &mut ids);
    assert_eq!(ids.len(), 6);
    // Родитель div без persistent id не создаёт отдельный scope для детей.
    for (i, (_, id)) in ids.iter().enumerate() {
        for (_, other) in &ids[i + 1..] {
            assert_ne!(id, other);
        }
    }
    let mut cloned = Vec::new();
    let cloned_nodes = nodes.to_vec();
    generated(&cloned_nodes, &mut cloned);
    assert_eq!(ids, cloned);
}
