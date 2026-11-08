//! Eligibility predicates retain pruning and the immediate parent at any depth.
use super::*;
use crate::style::computed::{
    Display, InheritedGroup, InheritedStyle, InheritedTextFlow, ResetBoxLayout, ResetTextBox,
    StyleGroup, TextEdge,
};

fn element(tag: &str, layout: ResetBoxLayout, trim: bool, children: Vec<Node>) -> Element {
    Element::synthetic(
        tag,
        Computed {
            inh: InheritedGroup::new(InheritedStyle {
                text_flow: StyleGroup::new(InheritedTextFlow {
                    text_box_under: TextEdge::Alphabetic,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            reset_box_layout: StyleGroup::new(layout),
            reset_text_box: StyleGroup::new(ResetTextBox {
                text_box_trim_end: trim,
                ..Default::default()
            }),
            ..Default::default()
        },
        Default::default(),
        children,
    )
}

fn block(children: Vec<Node>) -> Element {
    element(
        "div",
        ResetBoxLayout {
            display: Some(Display::Block),
            ..Default::default()
        },
        false,
        children,
    )
}

#[test]
fn deep_flow_predicate_checks_the_leaf_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            for float in [None, Some(1)] {
                let mut root = element(
                    "div",
                    ResetBoxLayout {
                        display: Some(Display::Block),
                        float,
                        ..Default::default()
                    },
                    false,
                    vec![],
                );
                for _ in 0..20_000 {
                    root = block(vec![Node::Element(root)]);
                }
                assert_eq!(flow_interior_plain(&root), float.is_none());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn flow_predicate_uses_immediate_parent_and_prunes_inline_atoms() {
    let clear = || {
        Node::Element(element(
            "br",
            ResetBoxLayout {
                clear: Some(1),
                ..Default::default()
            },
            false,
            vec![],
        ))
    };
    let layout = || ResetBoxLayout {
        display: Some(Display::Block),
        ..Default::default()
    };
    // Внешний срез не проходит через родителя без собственного среза.
    let root = element(
        "div",
        layout(),
        true,
        vec![Node::Element(block(vec![clear()]))],
    );
    assert!(flow_interior_plain(&root));
    // Для отложенного брата восстанавливается правило среза его родителя.
    let root = element(
        "div",
        layout(),
        true,
        vec![
            Node::Element(block(vec![Node::Text("text".into())])),
            clear(),
        ],
    );
    assert!(!flow_interior_plain(&root));
    let trimmed = element("div", layout(), true, vec![clear()]);
    assert!(!flow_interior_plain(&block(vec![Node::Element(trimmed)])));
    let floated = || {
        Node::Element(element(
            "div",
            ResetBoxLayout {
                float: Some(1),
                ..Default::default()
            },
            false,
            vec![],
        ))
    };
    let atomic = element(
        "span",
        ResetBoxLayout {
            display: Some(Display::InlineBlock),
            ..Default::default()
        },
        false,
        vec![floated()],
    );
    assert!(flow_interior_plain(&block(vec![Node::Element(atomic)])));
    let ordinary = element(
        "div",
        ResetBoxLayout {
            display: Some(Display::Contents),
            ..Default::default()
        },
        false,
        vec![floated()],
    );
    assert!(!flow_interior_plain(&block(vec![Node::Element(ordinary)])));
}
