//! HTML §4.15 / §4.10.18.5: disabledness, including the first legend exception.
//! Read the source tree only when a disabled/enabled selector asks for it;
//! no additional state or allocation is retained per element.

use super::{HTML, html, parent};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

fn is(handle: &Handle, tag: &str) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. }
        if &*name.ns == HTML && &*name.local == tag)
}

fn has_disabled(handle: &Handle) -> bool {
    matches!(&handle.data, NodeData::Element { attrs, .. }
        if attrs.borrow().iter().any(|a| &*a.name.local == "disabled"))
}

/// `None` means the element supports neither :disabled nor :enabled.
pub(crate) fn disabled(handle: &Handle) -> Option<bool> {
    let NodeData::Element { name, .. } = &handle.data else {
        return None;
    };
    if !html(handle) {
        return None;
    }
    match &*name.local {
        "option" => {
            return Some(
                has_disabled(handle)
                    || parent(handle).is_some_and(|p| is(&p, "optgroup") && has_disabled(&p))
                    || select_disabled(handle),
            );
        }
        "optgroup" => return Some(has_disabled(handle) || select_disabled(handle)),
        "button" | "input" | "select" | "textarea" | "fieldset" => {}
        _ => return None,
    }
    if has_disabled(handle) {
        return Some(true);
    }
    let mut child = handle.clone();
    while let Some(up) = parent(&child) {
        if is(&up, "fieldset") && has_disabled(&up) {
            let children = up.children.borrow();
            let in_first_legend = children
                .iter()
                .find(|c| is(c, "legend"))
                .is_some_and(|legend| Rc::ptr_eq(legend, &child));
            if !in_first_legend {
                return Some(true);
            }
        }
        child = up;
    }
    Some(false)
}

fn select_disabled(handle: &Handle) -> bool {
    let mut at = parent(handle);
    while let Some(node) = at {
        if is(&node, "select") {
            return disabled(&node) == Some(true);
        }
        at = parent(&node);
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn board_appearance_disabled_controls_and_first_legend() {
        let parsed = crate::dom::parse(
            "<style>:disabled{width:7px}:enabled{width:9px}</style>\
             <fieldset disabled id=f><legend><input id=a><fieldset disabled>\
             <input id=b></fieldset></legend><input id=c><legend><input id=d></legend>\
             <fieldset id=m></fieldset></fieldset><button disabled id=e></button><input id=g>\
             <div disabled id=h></div><select><optgroup disabled id=i>\
             <option id=j>x</option></optgroup><option id=k>x</option></select>\
             <select disabled><optgroup id=n><option id=o>x</option></optgroup></select>\
             <svg><rect disabled id=l /></svg>",
            "",
        );
        use crate::style::values::value::Len;
        for (id, expected) in [
            ("f", Some(7.)),
            ("a", Some(9.)),
            ("b", Some(7.)),
            ("c", Some(7.)),
            ("d", Some(7.)),
            ("e", Some(7.)),
            ("g", Some(9.)),
            ("h", None),
            ("i", Some(7.)),
            ("j", Some(7.)),
            ("k", Some(9.)),
            ("l", None),
            ("m", Some(7.)),
            ("n", Some(7.)),
            ("o", Some(7.)),
        ] {
            let node = crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some(id))
                .unwrap();
            assert_eq!(
                node.style.used().reset_box_layout.width,
                expected.map(Len::Px),
                "{id}"
            );
        }
    }
}
