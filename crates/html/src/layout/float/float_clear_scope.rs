//! Resolve clearance only for boxes to which CSS 2.1 section 9.5.2 applies.

use crate::dom::Node;
use crate::render::{inline_level_box, out_of_flow};
use crate::style::computed::Computed;

pub(crate) fn used(nodes: Vec<Node>, parent: &Computed) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|node| match node {
            Node::Element(mut element) => {
                if element.style.clear_inherit {
                    element.style.clear = parent.clear;
                }
                // Preserve the computed value for inheritance until this layout
                // copy. Inline boxes cannot acquire clearance; HTML br is a
                // separate legacy line-break mechanism, not a block's clearance.
                if inline_level_box(&element) && !out_of_flow(&element.style) && element.tag != "br"
                {
                    inherit_before_reset(&mut element.children, element.style.clear);
                    element.style.clear = None;
                }
                Node::Element(element)
            }
            other => other,
        })
        .collect()
}

fn inherit_before_reset(children: &mut [Node], parent_clear: Option<i8>) {
    for child in children {
        if let Node::Element(child) = child {
            if child.style.clear_inherit {
                child.style.clear = parent_clear;
                child.style.clear_inherit = false;
            }
            inherit_before_reset(&mut child.children, child.style.clear);
        }
    }
}
