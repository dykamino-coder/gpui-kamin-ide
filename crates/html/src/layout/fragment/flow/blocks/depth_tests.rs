//! Глубина DOM не ограничивает native-запись; отдельные продолжения F2 сохраняют границу.
use super::*;
use crate::dom::{Element, Node};
use crate::layout::fragment::flow::{independent_copy, vertical_copy};

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
fn deep_native_groups_record_the_innermost_box() {
    for mode in ["horizontal-tb", "vertical-lr", "vertical-rl"] {
        for body in [
            "<div style='height:20px'>text</div>",
            "<div style='display:grid'><div style='height:20px'></div></div>",
            "<table><tr><td><div style='height:20px'></div></td></tr></table>",
            "<div style='display:flex'><div style='height:20px'></div></div>",
        ] {
            let html = format!(
                "<div style='writing-mode:{mode}'>{}{body}{}</div>",
                "<div>".repeat(24),
                "</div>".repeat(24)
            );
            let tree = crate::dom::parse(&html, "");
            let e = root(&tree);
            let logical = if mode == "horizontal-tb" {
                assert!(crate::layout::fragment::push::measured_tree(e), "{body}");
                // F2-4: рекордер видит flex на любой глубине, но сплошная
                // копия ещё не заменяет consumed state его продолжения.
                assert_eq!(independent_copy(e), !body.contains("display:flex"));
                e.clone()
            } else {
                assert!(vertical_copy(e, mode == "vertical-rl"), "{mode}: {body}");
                crate::layout::fragment::flow::logical_copy(e, mode == "vertical-rl").unwrap()
            };
            fn last(e: &Element) -> u64 {
                e.children
                    .iter()
                    .rev()
                    .find_map(|n| match n {
                        Node::Element(k) => Some(last(k)),
                        _ => None,
                    })
                    .unwrap_or(e.node_id)
            }
            let b = BlockBox::collect(&logical, ShapeCx::COLUMNS, tree.values());
            assert!(b.ids().contains(&last(&logical)), "{mode}: {body}");
            assert!(b.ids().len() > 24, "{mode}: {body}");
        }
    }
}

#[test]
fn deep_separate_fragment_state_is_not_hidden_by_a_depth_limit() {
    for style in [
        "box-decoration-break:clone",
        "columns:2",
        "text-box-trim:trim-both",
    ] {
        let tree = crate::dom::parse(
            &format!(
                "<div>{}<div style='{style}'>text</div>{}</div>",
                "<div>".repeat(24),
                "</div>".repeat(24)
            ),
            "",
        );
        assert!(!independent_copy(root(&tree)), "{style}");
    }
}
