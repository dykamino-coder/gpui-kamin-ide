//! Метрики пустой строчной коробки (CSS 2.1 §10.8) учитываются только в абзаце, который уже
//! порождает строку: есть ли среди детей значащий текст в потоке и отличаются ли метрики
//! строки коробки (интерлиньяж, кегль, семейство) от родительских.

use crate::dom::Node;
use crate::style::computed::{Computed, Display, Position};

pub(super) fn has_text(children: &[Node]) -> bool {
    children.iter().any(|node| match node {
        Node::Text(text) => text.chars().any(|c| !matches!(c, ' ' | '\t' | '\n' | '\r')),
        Node::Element(element) => {
            element.style.used().reset_box_layout.display != Some(Display::None)
                && !matches!(
                    element.style.used().reset_positioned.position,
                    Some(Position::Absolute | Position::Fixed)
                )
                && element.style.used().reset_box_layout.float.unwrap_or(0) == 0
                && has_text(&element.children)
        }
    })
}

pub(super) fn different_metrics(style: &Computed, parent: &Computed) -> bool {
    style.inh.font.line_height != parent.inh.font.line_height
        || style.inh.font.font_size != parent.inh.font.font_size
        || style.inh.font.font_family != parent.inh.font.font_family
        || style.inh.font.monospace != parent.inh.font.monospace
}
