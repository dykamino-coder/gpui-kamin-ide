//! Идентичность соседних синтетических scroll-коробок в одном scope GPUI.

use crate::dom::{Node, parse};

#[test]
fn generated_scroll_boxes_have_distinct_stable_state_ids() {
    fn generated(
        nodes: &[Node],
        out: &mut Vec<(String, gpui::ElementId)>,
        vcx: crate::document::state::ValueCx<'_>,
    ) {
        for node in nodes {
            if let Node::Element(element) = node {
                if element.tag.starts_with("::") {
                    assert!(element.style.cascaded().overflow_y.is_some());
                    assert_eq!(element.node_id, 0);
                    assert!(element.marks.state_id.get().is_some());
                    out.push((
                        element.tag.clone(),
                        element.state_id("html-scroll-area", vcx),
                    ));
                }
                generated(&element.children, out, vcx);
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
    generated(nodes, &mut ids, parsed.doc.values());
    assert_eq!(ids.len(), 6);
    // Родитель div без persistent id не создаёт отдельный scope для детей.
    for (i, (_, id)) in ids.iter().enumerate() {
        for (_, other) in &ids[i + 1..] {
            assert_ne!(id, other);
        }
    }
    let mut cloned = Vec::new();
    let cloned_nodes = nodes.to_vec();
    generated(&cloned_nodes, &mut cloned, parsed.doc.values());
    assert_eq!(ids, cloned);
}

#[test]
fn lazy_synthetic_identity_survives_cloning_and_separates_roles_and_authored_ids() {
    use crate::dom::{Element, Marks};
    use crate::style::computed::Computed;
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    let synthetic = || Element::synthetic("div", Computed::default(), Marks::default(), vec![]);
    let first = synthetic();
    let clone_before_allocation = first.clone();
    let second = synthetic();
    let roles = [
        "html-scroll-area",
        "html-scroll-content",
        "html-resize",
        "html-animation",
        "html-transition",
    ];
    let mut keys = Vec::new();
    let mut authored = synthetic();
    for node_id in [1, 2, u64::MAX] {
        authored.node_id = node_id;
        for role in roles {
            keys.push(authored.state_id(role, vcx));
        }
    }
    for role in roles {
        let key = first.state_id(role, vcx);
        assert_eq!(key, clone_before_allocation.state_id(role, vcx));
        keys.push(key);
        keys.push(second.state_id(role, vcx));
    }
    for (i, key) in keys.iter().enumerate() {
        assert!(!keys[i + 1..].contains(key));
    }
    doc.begin_frame();
    assert_eq!(
        first.state_id(roles[0], vcx),
        clone_before_allocation.state_id(roles[0], vcx)
    );
    assert_eq!((first.node_id, second.node_id), (0, 0));
}
