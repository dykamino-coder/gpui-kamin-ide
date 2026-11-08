//! Сборка дерева: текст для замера первой строки с семантикой разрывов
//! `white-space` (CSS Text 3 §4.1). Сегментные разрывы источника становятся
//! пробелами, если не сохранены; первую форматированную строку завершает только
//! сохранённый разрыв или `<br>`. Отдельно собирается текст с жёсткими
//! разрывами для замера полной высоты обёртки.

use crate::dom::Node;

/// CSS Text 3 §4.1: source segment breaks become spaces unless preserved.
/// Only a preserved segment break or a br ends the first formatted line.
pub(super) fn gather(nodes: &[Node], preserve_newlines: bool, out: &mut String) -> bool {
    for (node, preserve) in text_nodes(nodes, preserve_newlines) {
        match node {
            Node::Text(text) => {
                if preserve && let Some(cut) = text.find('\n') {
                    out.push_str(&text[..cut]);
                    return true;
                }
                out.push_str(text);
            }
            Node::Element(element) if element.tag == "br" => return true,
            Node::Element(_) => {}
        }
    }
    false
}

/// Preserve hard breaks for the wrapper's total-height measurement.
pub(super) fn lines(nodes: &[Node], preserve: bool, out: &mut String) {
    for (node, preserve) in text_nodes(nodes, preserve) {
        match node {
            Node::Text(text) if preserve => out.push_str(text),
            Node::Text(text) => out.push_str(&text.replace(['\n', '\r'], " ")),
            Node::Element(e) if e.tag == "br" => out.push('\n'),
            Node::Element(_) => {}
        }
    }
}

fn text_nodes(nodes: &[Node], preserve: bool) -> impl Iterator<Item = (&Node, bool)> {
    crate::dom::traversal::read_inherited(nodes, preserve, |element, parent| {
        (element.tag != "br").then(|| {
            element
                .style
                .cascaded()
                .inh
                .text_flow
                .preserve_newlines
                .unwrap_or(*parent)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_indentation_does_not_end_a_collapsing_first_line() {
        let nodes = [Node::Text("\n    first\n    line".into())];
        let mut text = String::new();
        assert!(!gather(&nodes, false, &mut text));
        assert_eq!(text, "\n    first\n    line");
        text.clear();
        assert!(gather(&nodes, true, &mut text));
        assert!(text.is_empty());
    }
    #[test]
    fn deep_text_preserves_scoped_breaks_and_restores_sibling_state() {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                use crate::dom::Element;
                use crate::style::computed::{
                    Computed, InheritedGroup, InheritedStyle, InheritedTextFlow, StyleGroup,
                };
                let element = |preserve_newlines, children| {
                    Node::Element(Element::synthetic(
                        "span",
                        Computed {
                            inh: InheritedGroup::new(InheritedStyle {
                                text_flow: StyleGroup::new(InheritedTextFlow {
                                    preserve_newlines,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        Default::default(),
                        children,
                    ))
                };
                let mut nodes = vec![Node::Text("deep\ntext".into())];
                for _ in 0..20_000 {
                    nodes = vec![element(None, nodes)];
                }
                let nodes = [
                    element(Some(false), nodes),
                    Node::Text("tail\nstop".into()),
                    Node::Text("ignored".into()),
                ];
                let mut text = String::new();
                assert!(gather(&nodes, true, &mut text));
                assert_eq!(text, "deep\ntexttail");
                text.clear();
                lines(&nodes, true, &mut text);
                assert_eq!(text, "deep texttail\nstopignored");
                let nodes = [
                    element(Some(true), vec![Node::Text("first\nsecond".into())]),
                    Node::Text("tail\nend".into()),
                ];
                text.clear();
                assert!(gather(&nodes, false, &mut text));
                assert_eq!(text, "first");
                text.clear();
                lines(&nodes, false, &mut text);
                assert_eq!(text, "first\nsecondtail end");
                let nodes = [
                    Node::Element(Element::synthetic(
                        "br",
                        Default::default(),
                        Default::default(),
                        vec![Node::Text("pruned".into())],
                    )),
                    Node::Text("after".into()),
                ];
                text.clear();
                assert!(gather(&nodes, false, &mut text));
                assert!(text.is_empty());
                lines(&nodes, false, &mut text);
                assert_eq!(text, "\nafter");
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
