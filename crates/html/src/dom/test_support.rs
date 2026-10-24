//! Поиск элемента в тестовом дереве в порядке документа.

use super::{Element, Node};

pub(crate) fn find<'a>(
    nodes: &'a [Node],
    matches: &impl Fn(&Element) -> bool,
) -> Option<&'a Element> {
    nodes.iter().find_map(|node| match node {
        Node::Element(element) if matches(element) => Some(element),
        Node::Element(element) => find(&element.children, matches),
        Node::Text(_) => None,
    })
}
