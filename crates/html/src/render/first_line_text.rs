//! Сборка дерева: текст для замера первой строки с семантикой разрывов
//! `white-space` (CSS Text 3 §4.1). Сегментные разрывы источника становятся
//! пробелами, если не сохранены; первую форматированную строку завершает только
//! сохранённый разрыв или `<br>`. Отдельно собирается текст с жёсткими
//! разрывами для замера полной высоты обёртки.

use crate::dom::Node;

/// CSS Text 3 §4.1: source segment breaks become spaces unless preserved.
/// Only a preserved segment break or a br ends the first formatted line.
pub(super) fn gather(nodes: &[Node], preserve_newlines: bool, out: &mut String) -> bool {
    for node in nodes {
        match node {
            Node::Text(text) => {
                if preserve_newlines && let Some(cut) = text.find('\n') {
                    out.push_str(&text[..cut]);
                    return true;
                }
                out.push_str(text);
            }
            Node::Element(element) if element.tag == "br" => return true,
            Node::Element(element) => {
                let preserve = element.style.preserve_newlines.unwrap_or(preserve_newlines);
                if gather(&element.children, preserve, out) {
                    return true;
                }
            }
        }
    }
    false
}

/// Preserve hard breaks for the wrapper's total-height measurement.
pub(super) fn lines(nodes: &[Node], preserve: bool, out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) if preserve => out.push_str(text),
            Node::Text(text) => out.push_str(&text.replace(['\n', '\r'], " ")),
            Node::Element(e) if e.tag == "br" => out.push('\n'),
            Node::Element(e) => lines(
                &e.children,
                e.style.preserve_newlines.unwrap_or(preserve),
                out,
            ),
        }
    }
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
}
