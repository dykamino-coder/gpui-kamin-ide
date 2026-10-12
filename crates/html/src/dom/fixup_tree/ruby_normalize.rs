//! CSS Ruby box fixup: inlinify in-flow children before constructing ruby levels.

use crate::dom::{BLOCK_TAGS, Node};
use crate::style::computed::{Display, Position};

/// CSS Ruby 1 §2.2 and CSS Display 4 §2.7: an inline propagates
/// inlinification, while a flow root keeps its own block formatting context.
/// Blink style_adjuster.cc:804 also clears floats in this context.
pub(crate) fn inlinify(nodes: &mut [Node]) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        if matches!(e.style.position, Some(Position::Absolute | Position::Fixed)) {
            continue;
        }
        e.style.float = None;
        let role = super::ruby_boxes::ruby_box_role(&e.tag, &e.style);
        if role.is_some() {
            if e.style.display == Some(Display::Block) {
                e.style.display = None;
                e.style.inline_display = Some(true);
            }
            inlinify(&mut e.children);
            continue;
        }
        let block = e.style.display == Some(Display::Block)
            || (e.style.display.is_none()
                && e.style.inline_display != Some(true)
                && BLOCK_TAGS.contains(&e.tag.as_str()));
        e.style.display = match e.style.display {
            _ if block => Some(Display::InlineBlock),
            Some(Display::Flex) => Some(Display::InlineFlex),
            Some(Display::Grid) => Some(Display::InlineGrid),
            Some(Display::Table) => Some(Display::InlineTable),
            other => other,
        };
        if matches!(
            e.style.display,
            None | Some(Display::Contents | Display::ListItem)
        ) {
            e.style.inline_display = Some(true);
            inlinify(&mut e.children);
        }
    }
}
