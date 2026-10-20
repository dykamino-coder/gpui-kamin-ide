//! Деление DOM-узлов по байтовому смещению в тексте поддерева.

use super::gather_text;
use crate::dom::Node;

/// Разрезать список узлов по смещению в их общем тексте.
///
/// Смещение считается по тому же тексту, что уходит в переносчик, поэтому
/// элементы режутся вместе с ним: `<b>` на границе разреза становится двумя.
pub(crate) fn split_nodes(nodes: &[Node], at: usize) -> (Vec<Node>, Vec<Node>) {
    let mut before = vec![];
    let mut after = vec![];
    let mut seen = 0usize;
    for node in nodes {
        if seen >= at {
            after.push(node.clone());
            continue;
        }
        match node {
            Node::Text(t) => {
                let len = t.len();
                if seen + len <= at {
                    before.push(node.clone());
                } else {
                    // Режем по границе символа, ближайшей к месту разреза.
                    let mut cut = at - seen;
                    while cut < t.len() && !t.is_char_boundary(cut) {
                        cut += 1;
                    }
                    if cut > 0 {
                        before.push(Node::Text(t[..cut].to_string()));
                    }
                    if cut < t.len() {
                        after.push(Node::Text(t[cut..].to_string()));
                    }
                }
                seen += len;
            }
            Node::Element(e) => {
                let mut text = String::new();
                gather_text(&e.children, &mut text);
                let len = text.len();
                if seen + len <= at {
                    before.push(node.clone());
                } else {
                    let (head, tail) = split_nodes(&e.children, at - seen);
                    let mut a = e.clone();
                    a.children = head;
                    let mut b = e.clone();
                    b.children = tail;
                    before.push(Node::Element(a));
                    after.push(Node::Element(b));
                }
                seen += len;
            }
        }
    }
    (before, after)
}
