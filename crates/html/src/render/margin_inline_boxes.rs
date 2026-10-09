//! Build undecorated inline continuations before collapsing ancestor block margins.

use super::{Display, Node, contains_block, inline_floats, out_of_flow, real_inline, replaced_tag};
use super::{split_block_in_inline, wrap_anon_tables};

/// CSS 2.1 sections 9.2.1.1 and 8.3.1: adjoining margins belong to the
/// generated block boxes, including blocks hoisted out of inline ancestors.
/// Preparing descendants first lets the parent's leading/trailing chains
/// see those boxes instead of an inline node that prematurely stops them.
pub(super) fn prepare(nodes: &mut [Node]) {
    for node in nodes {
        let Node::Element(element) = node else {
            continue;
        };
        if replaced_tag(element) {
            continue;
        }
        prepare(&mut element.children);
        if real_inline(element)
            || matches!(
                element.style.display,
                Some(Display::Flex)
                    | Some(Display::InlineFlex)
                    | Some(Display::Grid)
                    | Some(Display::InlineGrid)
                    | Some(Display::GridLanes)
            )
            || !eligible(&element.children)
        {
            continue;
        }
        if element.children.iter().any(|node| {
            matches!(node, Node::Element(child) if real_inline(child)
                && !out_of_flow(&child.style) && contains_block(&child.children))
        }) {
            element.children = split_block_in_inline(&wrap_anon_tables(&element.children));
        }
    }
}

fn eligible(nodes: &[Node]) -> bool {
    nodes.iter().all(|node| match node {
        Node::Element(child)
            if real_inline(child)
                && !out_of_flow(&child.style)
                && contains_block(&child.children) =>
        {
            // Decorations, positioning and bidi need their original inline
            // ancestry until fragment painting; keep that established path.
            inline_floats::transparent(child) && eligible(&child.children)
        }
        _ => true,
    })
}
