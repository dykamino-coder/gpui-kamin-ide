//! Keep empty inline metrics in a paragraph that already generates a line.

use crate::computed::{Computed, Display, Position};
use crate::dom::Node;

pub(super) fn has_text(children: &[Node]) -> bool {
    children.iter().any(|node| match node {
        Node::Text(text) => text.chars().any(|c| !matches!(c, ' ' | '\t' | '\n' | '\r')),
        Node::Element(element) => {
            element.style.display != Some(Display::None)
                && !matches!(
                    element.style.position,
                    Some(Position::Absolute | Position::Fixed)
                )
                && element.style.float.unwrap_or(0) == 0
                && has_text(&element.children)
        }
    })
}

pub(super) fn different_metrics(style: &Computed, parent: &Computed) -> bool {
    style.line_height != parent.line_height
        || style.font_size != parent.font_size
        || style.font_family != parent.font_family
        || style.monospace != parent.monospace
}
