//! Замещаемые коробки и разрывы — листья IFC, включая вложенный fallback.

use super::*;
use crate::dom::Node;

fn element(tag: &str, children: Vec<Node>) -> Node {
    Node::Element(crate::layout::table::anon::anon_element(tag, children))
}

#[test]
fn replaced_descendants_do_not_create_html_inline_flows() {
    for tag in [
        "img", "svg", "canvas", "video", "embed", "object", "iframe", "input",
    ] {
        let mut tree = super::super::BoxTree::default();
        tree.publish(&[element(
            "main",
            vec![
                Node::Text("before".into()),
                element(
                    tag,
                    vec![element("span", vec![Node::Text("fallback".into())])],
                ),
                Node::Text("after".into()),
            ],
        )]);
        let flows: Vec<_> = tree
            .arena
            .iter()
            .filter_map(|(_, r)| r.inline.as_ref())
            .collect();
        assert_eq!(flows.len(), 1, "{tag} descendants are not HTML flow owners");
        assert_eq!(flows[0].text, "beforeafter", "{tag}");
        assert_eq!(flows[0].items.len(), 3, "{tag} is represented by one leaf");
    }
}

#[test]
fn break_descendants_cannot_leak_into_the_parent_flow() {
    for (tag, kind) in [("br", ItemKind::HardBreak), ("wbr", ItemKind::Opportunity)] {
        let mut tree = super::super::BoxTree::default();
        tree.publish(&[element(
            "main",
            vec![
                element(tag, vec![Node::Text("ignored".into())]),
                Node::Text("after".into()),
            ],
        )]);
        let flow = tree.get(tree.first).unwrap().inline.as_ref().unwrap();
        assert_eq!(flow.text, "after");
        assert_eq!(flow.items[0].kind, kind);
        assert_eq!(flow.items.len(), 2);
        assert_eq!(
            tree.arena
                .iter()
                .filter(|(_, r)| r.inline.is_some())
                .count(),
            1
        );
    }
}

#[test]
fn floats_keep_their_order_and_do_not_share_absolute_position_markers() {
    let doc = crate::document::Document::builder(
        "<main>before<i style='float:left'>left</i><b style='float:right'>right</b><span style='position:absolute;float:left'>overlay</span>after</main>",
        "",
    ).build().unwrap();
    let content = super::tests::flow(&doc, "main");
    assert_eq!(content.text, "beforeafter");
    assert_eq!(
        content
            .items
            .iter()
            .map(|item| &item.kind)
            .collect::<Vec<_>>(),
        [
            &ItemKind::Text { start: 0, end: 6 },
            &ItemKind::Float,
            &ItemKind::Float,
            &ItemKind::OutOfFlow,
            &ItemKind::Text { start: 6, end: 11 },
        ],
    );
    for (tag, text) in [("i", "left"), ("b", "right"), ("span", "overlay")] {
        assert_eq!(super::tests::flow(&doc, tag).text, text);
    }
}
