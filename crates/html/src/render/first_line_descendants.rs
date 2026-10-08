//! Route `::first-line` styling into the first in-flow block descendant.
//!
//! CSS 2 §5.12.1: «The "first formatted line" of an element may occur inside
//! a block-level descendant in the same flow (i.e., a block-level descendant
//! that is not out-of-flow due to floating or positioning)»; the fictitious
//! `::first-line` tag sequence is placed inside that descendant
//! (css-pseudo-4 §first-line-inheritance). Only the declarations of the
//! pseudo-element travel: everything it does not set stays with the text's
//! own style, as for `first_letter_descendants`.

use super::{Computed, Display, Node, blank_text, block_level_in_flow, out_of_flow, table_box};

pub(super) fn route(mut nodes: Vec<Node>, parent: &Computed) -> Vec<Node> {
    let Some(first) = parent.first_line_own.as_deref() else {
        return nodes;
    };
    for node in &mut nodes {
        match node {
            Node::Text(t) if blank_text(t) => continue,
            Node::Element(e) if out_of_flow(&e.style) => continue,
            Node::Element(e)
                if block_level_in_flow(e)
                    && !table_box(e)
                    && matches!(
                        e.style.display,
                        None | Some(Display::Block | Display::ListItem)
                    )
                    && e.first_line.is_none() =>
            {
                e.style.first_line_own = Some(Box::new(first.clone()));
                e.first_line = Some(first.clone());
            }
            _ => {}
        }
        // A later sibling cannot supply the first formatted line.
        break;
    }
    nodes
}
