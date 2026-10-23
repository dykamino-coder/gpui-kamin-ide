//! Рост разложенного вертикального блока по логической блочной оси.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::fragment::types::StackAxis;
use crate::style::values::value::{Len, px_lossy_zero};

pub(super) fn push(
    parent: &mut Element,
    id: u64,
    grow: f32,
    forced: bool,
    axis: StackAxis,
    vcx: ValueCx<'_>,
) -> bool {
    let reverse = axis == StackAxis::VerticalRl;
    if let Some(at) = parent
        .children
        .iter()
        .position(|n| matches!(n, Node::Element(k) if k.node_id == id))
    {
        if !forced {
            let Node::Element(child) = &mut parent.children[at] else {
                unreachable!()
            };
            let margin = if reverse {
                &mut child.style.used_mut().margin.right
            } else {
                &mut child.style.used_mut().margin.left
            };
            *margin = Some(Len::Px(px_lossy_zero(*margin) + grow));
            return true;
        }
        if !matches!(&parent.children[at], Node::Element(k) if crate::layout::fragment::breaks::edge_break(k, false, vcx))
        {
            return false;
        }
        let previous = parent.children[..at]
            .iter_mut()
            .rev()
            .find(|n| !crate::dom::whitespace::is_blank(n));
        let tail = if let Some(Node::Element(k)) = previous {
            if reverse {
                k.style.used_mut().margin.left.take()
            } else {
                k.style.used_mut().margin.right.take()
            }
        } else {
            None
        };
        let mut spacer = crate::layout::table::anon::anon_element("div", Vec::new());
        spacer.node_id = id ^ crate::layout::fragment::flow::blocks::STRUT_ID;
        spacer.style.used_mut().width = Some(Len::Px(grow));
        spacer.style.used_mut().flex_shrink = Some(0.0);
        if reverse {
            spacer.style.used_mut().margin.left = tail;
        } else {
            spacer.style.used_mut().margin.right = tail;
        }
        parent.children.insert(at, Node::Element(spacer));
        if let Some(Len::Px(width)) = parent.style.computed().width {
            parent.style.used_mut().width = Some(Len::Px(width + grow));
        }
        return true;
    }
    parent.children.iter_mut().any(|n| match n {
        Node::Element(k) => push(k, id, grow, forced, axis, vcx),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("authored div")
    }

    #[test]
    fn growth_uses_the_logical_start_margin_in_both_directions() {
        for (axis, reverse) in [
            (StackAxis::VerticalRl, true),
            (StackAxis::VerticalLr, false),
        ] {
            let tree = crate::dom::parse(
                "<div><div style='margin-left:3px;margin-right:7px;width:20px'></div></div>",
                "",
            );
            let mut host = host(&tree).clone();
            let Node::Element(child) = &host.children[0] else {
                panic!("child")
            };
            let id = child.node_id;
            assert!(push(&mut host, id, 11.0, false, axis, tree.values()));
            let Node::Element(child) = &host.children[0] else {
                panic!("child")
            };
            assert_eq!(child.style.computed().width, Some(Len::Px(20.0)));
            assert_eq!(
                child.style.computed().margin.left,
                Some(Len::Px(if reverse { 3.0 } else { 14.0 }))
            );
            assert_eq!(
                child.style.computed().margin.right,
                Some(Len::Px(if reverse { 18.0 } else { 7.0 }))
            );
        }
    }

    #[test]
    fn forced_spacer_preserves_the_previous_logical_tail() {
        for (axis, reverse) in [
            (StackAxis::VerticalRl, true),
            (StackAxis::VerticalLr, false),
        ] {
            let tree = crate::dom::parse(
                "<div style='width:80px'><div style='margin-left:5px;margin-right:9px;width:20px'></div><div style='break-before:column;width:30px'></div></div>",
                "",
            );
            let mut host = host(&tree).clone();
            let Node::Element(next) = &host.children[1] else {
                panic!("next")
            };
            let id = next.node_id;
            assert!(push(&mut host, id, 13.0, true, axis, tree.values()));
            let Node::Element(previous) = &host.children[0] else {
                panic!("previous")
            };
            let Node::Element(spacer) = &host.children[1] else {
                panic!("spacer")
            };
            let Node::Element(next) = &host.children[2] else {
                panic!("next")
            };
            assert_eq!(next.node_id, id);
            assert_eq!(spacer.style.computed().width, Some(Len::Px(13.0)));
            assert_eq!(host.style.computed().width, Some(Len::Px(93.0)));
            let (before, after) = if reverse {
                (
                    previous.style.computed().margin.left,
                    spacer.style.computed().margin.left,
                )
            } else {
                (
                    previous.style.computed().margin.right,
                    spacer.style.computed().margin.right,
                )
            };
            assert_eq!(before, None);
            assert_eq!(after, Some(Len::Px(if reverse { 5.0 } else { 9.0 })));
        }
    }
}
