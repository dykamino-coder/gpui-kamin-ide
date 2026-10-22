//! Поддеревья flex, чьи переносы выражаются измеренными коробками и ростом Regrow.
use super::{flex_container, grid_tree, table_tree};
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::{clone_dec, monolith_blockified};
use crate::layout::fragment::flow::box_children::children;
use crate::style::computed::Display;

pub(crate) fn flex_tree(e: &Element) -> bool {
    fn eligible(e: &Element, item: bool) -> bool {
        if e.style.wm().vertical()
            || clone_dec(e).is_some()
            || e.style.webkit_box == Some(true)
            || crate::layout::multicol::spanner::multicol_container(&e.style)
            || !crate::layout::page::paged::visible_overflow(&e.style)
        {
            return false;
        }
        if grid_tree(e) || table_tree(e) || (!flex_container(e) && super::plain_tree(e)) {
            return true;
        }
        let flex = flex_container(e);
        if !(flex
            || matches!(
                e.style.display,
                None | Some(Display::Block | Display::ListItem)
            )
            || item && e.style.display == Some(Display::InlineBlock))
        {
            return false;
        }
        children(&e.children).all(|n| match n {
            // OOF не участвует в членстве линии и не создаёт разрыв потока.
            Node::Element(k) if out_of_flow(&k.style) => true,
            Node::Element(k) if !k.inline || flex => monolith_blockified(k) || eligible(k, flex),
            n => is_blank(n),
        })
    }
    fn contains(e: &Element) -> bool {
        flex_container(e)
            || children(&e.children)
                .any(|n| matches!(n, Node::Element(k) if !out_of_flow(&k.style) && contains(k)))
    }
    contains(e) && eligible(e, false)
}
