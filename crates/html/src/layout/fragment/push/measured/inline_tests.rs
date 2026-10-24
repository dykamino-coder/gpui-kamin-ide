//! Атомарный inline сохраняет Class B продолжение; блокификация сетки — свой рекордер.
use super::*;
use crate::dom::{Element, Node};

fn root(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored root")
}

#[test]
fn atomic_inline_and_blockified_grid_items_keep_distinct_routes() {
    for display in ["inline-block", "inline-flex", "inline-grid", "inline-table"] {
        let tree = crate::dom::parse(
            &format!(
                "<div style='padding-bottom:50px'><div><div style='display:{display};height:50px'></div></div></div>"
            ),
            "",
        );
        let e = root(&tree);
        assert!(!measured_tree(e), "{display}");
        assert!(
            !crate::layout::fragment::flow::independent_copy(e),
            "{display}"
        );
        assert!(!measured_push(e, Some(100.0), false), "{display}");
    }
    for body in [
        "<div><div style='height:20px'></div></div>",
        "<div style='display:grid'><div style='display:inline-block;height:20px'></div></div>",
    ] {
        let tree = crate::dom::parse(body, "");
        assert!(measured_tree(root(&tree)), "{body}");
    }
}
