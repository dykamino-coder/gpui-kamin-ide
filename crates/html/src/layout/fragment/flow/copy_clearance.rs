//! Соседи одного BFC измеряются вместе, пока между ними живут float exclusions.
use crate::dom::{Element, Node};
use crate::layout::box_kind::{out_of_flow, own_context};
use crate::style::computed::Display;

pub(crate) fn copy_flow(nodes: &[Node]) -> std::borrow::Cow<'_, [Node]> {
    let mut floats = false;
    for n in super::box_children::children(nodes) {
        if floats && matches!(n, Node::Element(_)) {
            // Обычная обёртка не заводит BFC: collapse и clear видят те же bands.
            return std::borrow::Cow::Owned(vec![Node::Element(
                crate::layout::table::anon::anon_element("anon-block", nodes.to_vec()),
            )]);
        }
        floats |= matches!(n, Node::Element(e) if reaches(e));
    }
    std::borrow::Cow::Borrowed(nodes)
}

pub(crate) fn reaches(e: &Element) -> bool {
    if e.style.used().display == Some(Display::None) {
        return false;
    }
    let floated = e.style.used().float.is_some_and(|f| f != 0);
    if floated {
        return true;
    }
    if out_of_flow(&e.style.used()) {
        return false;
    }
    // CSS 2.1 §9.4.1: чужой BFC не экспортирует свои float bands.
    !own_context(e)
        && e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if reaches(k)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusions_cross_siblings_only_within_the_same_bfc() {
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
                true,
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
                matches!(
                    copy_flow(&find(&doc).unwrap().children),
                    std::borrow::Cow::Owned(_)
                ),
                crosses,
                "{body}"
            );
        }
    }
}
