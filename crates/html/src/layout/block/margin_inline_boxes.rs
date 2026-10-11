//! Build undecorated inline continuations before collapsing ancestor block margins.

use crate::dom::Node;
use crate::layout::float::inline_floats;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::table::anon::wrap_anon_tables;
use crate::render::{
    contains_block, out_of_flow, real_inline, replaced_tag, split_block_in_inline,
};
use crate::style::computed::Display;

/// CSS 2.1 sections 9.2.1.1 and 8.3.1: adjoining margins belong to the
/// generated block boxes, including blocks hoisted out of inline ancestors.
/// Preparing descendants first lets the parent's leading/trailing chains
/// see those boxes instead of an inline node that prematurely stops them.
pub(super) fn prepare(nodes: &mut [Node]) {
    for node in nodes {
        let Node::Element(element) = node else {
            continue;
        };
        // The column stack plans each in-flow child of a multicol container
        // as a separate fragmentable box and does not model a float of one
        // child intruding into the lines of the next. Splitting an inline
        // around a block that carries a float would move the float and the
        // lines that wrap around it into different stack children
        // (block-in-inline-012), so such content keeps its original tree.
        // Without floats the split is needed: margins of the hoisted block
        // adjoin the column break and are truncated there (css-break-3 §5.2,
        // margin-at-break-003…005).
        if replaced_tag(element)
            || (multicol_container(&element.style) && floats_inside(&element.children))
        {
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

/// Whether any in-flow descendant of `nodes` is floated.
fn floats_inside(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(e) => e.style.float.is_some_and(|f| f != 0) || floats_inside(&e.children),
        Node::Text(_) => false,
    })
}
