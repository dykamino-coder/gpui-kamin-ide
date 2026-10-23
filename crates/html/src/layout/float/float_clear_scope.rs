//! Используемое значение `clear` на копии дерева для раскладки: клиренс получают только
//! коробки, к которым применима CSS 2.1 §9.5.2. У строчной коробки `clear` снимается (кроме
//! `<br>` — отдельного механизма разрыва строки), но до снятия вычисленное значение
//! наследуется детям с `clear_inherit`.

use crate::dom::Node;
use crate::layout::box_kind::{inline_level_box, out_of_flow};
use crate::style::computed::Computed;

pub(super) fn used(nodes: Vec<Node>, parent: &Computed) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|node| match node {
            Node::Element(mut element) => {
                if element.style.used().clear_inherit {
                    element.style.used_mut().clear = parent.clear;
                }
                // Preserve the computed value for inheritance until this layout
                // copy. Inline boxes cannot acquire clearance; HTML br is a
                // separate legacy line-break mechanism, not a block's clearance.
                if inline_level_box(&element)
                    && !out_of_flow(&element.style.used())
                    && element.tag != "br"
                {
                    inherit_before_reset(&mut element.children, element.style.used().clear);
                    element.style.used_mut().clear = None;
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
            if child.style.used().clear_inherit {
                child.style.used_mut().clear = parent_clear;
                child.style.used_mut().clear_inherit = false;
            }
            inherit_before_reset(&mut child.children, child.style.used().clear);
        }
    }
}
