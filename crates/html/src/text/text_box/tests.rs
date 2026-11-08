//! Первая и последняя строка: глубокий спуск, владелец и границы поиска.

use super::*;
use crate::dom::Element;
use crate::style::computed::{ResetBoxLayout, ResetTextBox, StyleGroup};

fn block(id: u64, layout: ResetBoxLayout, children: Vec<Node>) -> Element {
    let mut element = Element::synthetic(
        "div",
        Computed {
            reset_box_layout: StyleGroup::new(layout),
            ..Default::default()
        },
        Default::default(),
        children,
    );
    element.node_id = id;
    element
}

#[test]
fn deep_line_search_keeps_the_line_owner_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let layout = || ResetBoxLayout {
                display: Some(Display::Block),
                ..Default::default()
            };
            let leaf = block(17, layout(), vec![Node::Text("line".into())]);
            let mut nodes = vec![Node::Element(leaf)];
            for depth in 0..20_000 {
                nodes = vec![Node::Element(block(100 + depth, layout(), nodes))];
            }
            let values = crate::values::DocumentValues::detached(Default::default());
            let inherited = BoxStyle::default();
            for start in [true, false] {
                let (_, owner) = text_box_line_style(&nodes, &inherited, start, 3, values.values())
                    .expect("deep line");
                assert_eq!(owner, 17);
            }
            assert!(has_text(&nodes));
            assert!(!has_text(&[Node::Text(" \n\t".into())]));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn line_search_preserves_contents_and_formatting_boundaries() {
    let values = crate::values::DocumentValues::detached(Default::default());
    let inherited = BoxStyle::default();
    let find = |nodes: &[Node], start| {
        text_box_line_style(nodes, &inherited, start, 3, values.values()).map(|(_, owner)| owner)
    };
    let line = || Node::Text("line".into());
    let contents = |children| {
        Node::Element(block(
            9,
            ResetBoxLayout {
                display: Some(Display::Contents),
                ..Default::default()
            },
            children,
        ))
    };
    assert_eq!(find(&[contents(vec![line()])], true), Some(3));
    // Выбранное contents-поддерево пусто: последующие братья его не заменяют.
    assert_eq!(find(&[contents(vec![]), line()], true), None);
    assert_eq!(find(&[line(), contents(vec![])], false), None);
    for display in [
        Display::Flex,
        Display::Grid,
        Display::Table,
        Display::GridLanes,
    ] {
        let nodes = [Node::Element(block(
            9,
            ResetBoxLayout {
                display: Some(display),
                ..Default::default()
            },
            vec![line()],
        ))];
        assert_eq!(find(&nodes, true), None);
        assert_eq!(find(&nodes, false), None);
    }
    let nodes = [Node::Element(block(
        9,
        ResetBoxLayout {
            display: Some(Display::Block),
            padding: crate::style::computed::Sides {
                top: Some(crate::style::values::value::Len::Px(2.0)),
                ..Default::default()
            },
            ..Default::default()
        },
        vec![line()],
    ))];
    assert_eq!(find(&nodes, true), None);
    assert_eq!(find(&nodes, false), Some(9));
    let mut trimmed = block(
        9,
        ResetBoxLayout {
            display: Some(Display::Block),
            ..Default::default()
        },
        vec![line()],
    );
    trimmed.style = Computed {
        reset_box_layout: StyleGroup::new(ResetBoxLayout {
            display: Some(Display::Block),
            ..Default::default()
        }),
        reset_text_box: StyleGroup::new(ResetTextBox {
            text_box_trim_start: true,
            ..Default::default()
        }),
        ..Default::default()
    }
    .into();
    assert_eq!(find(&[Node::Element(trimmed)], true), None);
}
