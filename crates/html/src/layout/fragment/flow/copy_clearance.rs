//! Clearance между независимыми копиями требует общего состояния float bands.
use crate::dom::{Element, Node};
use crate::layout::box_kind::{out_of_flow, own_context};
use crate::style::computed::Display;

/// TRANSITIONAL(F2-1): Отдельная копия не видит float bands предыдущего соседа; удалить отказ в P06 после переноса состояния BFC между фрагментами.
pub(crate) fn copy_clearance(nodes: &[Node]) -> bool {
    fn reaches(e: &Element, float: bool) -> bool {
        if e.style.used().display == Some(Display::None) {
            return false;
        }
        let floated = e.style.used().float.is_some_and(|f| f != 0);
        if float && floated {
            return true;
        }
        if out_of_flow(&e.style.used()) {
            return false;
        }
        if !float && e.style.used().clear.is_some() {
            return true;
        }
        // CSS 2.1 §9.4.1: чужой BFC не экспортирует свои float bands.
        !own_context(e)
            && e.children
                .iter()
                .any(|n| matches!(n, Node::Element(k) if reaches(k, float)))
    }
    let mut floats = false;
    for n in super::box_children::children(nodes) {
        if let Node::Element(e) = n {
            if floats && reaches(e, false) {
                return true;
            }
            floats |= reaches(e, true);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clearance_only_crosses_copies_in_the_same_bfc() {
        for (body, crosses) in [
            (
                "<div><div style='float:left;height:250px'></div></div><div style='clear:left'></div>",
                true,
            ),
            (
                "<div><div style='float:left'></div><div style='clear:both'></div></div>",
                false,
            ),
            (
                "<div style='overflow:hidden'><div style='float:left'></div></div><div style='clear:left'></div>",
                false,
            ),
            (
                "<div><div style='float:left'></div></div><div style='overflow:hidden'><div style='clear:left'></div></div>",
                false,
            ),
        ] {
            let doc = crate::dom::parse(&format!("<div>{body}</div>"), "");
            fn find(nodes: &[Node]) -> Option<&Element> {
                nodes.iter().find_map(|n| match n {
                    Node::Element(e) if e.tag == "div" => Some(e),
                    Node::Element(e) => find(&e.children),
                    _ => None,
                })
            }
            assert_eq!(
                copy_clearance(&find(&doc).unwrap().children),
                crosses,
                "{body}"
            );
        }
    }
}
