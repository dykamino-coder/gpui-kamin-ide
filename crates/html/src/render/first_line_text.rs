//! Collect first-line measurement text using CSS white-space break semantics.

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
