//! Поддеревья flex, чьи переносы выражаются измеренными коробками и ростом Regrow.
use super::{flex_container, grid_tree, table_tree};
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::{clone_dec, monolith_blockified};
use crate::style::computed::Display;

pub(crate) fn flex_tree(e: &Element, depth: u8) -> bool {
    fn eligible(e: &Element, depth: u8, item: bool) -> bool {
        if e.style.wm().vertical()
            || clone_dec(e).is_some()
            || e.style.webkit_box == Some(true)
            || crate::layout::multicol::spanner::multicol_container(&e.style)
            || !crate::layout::page::paged::visible_overflow(&e.style)
        {
            return false;
        }
        if grid_tree(e, depth)
            || table_tree(e, depth)
            || (!flex_container(e) && crate::layout::fragment::probe::plain_block_tree(e, depth))
        {
            return true;
        }
        if depth == 0 {
            return e.children.iter().all(is_blank);
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
        e.children.iter().all(|n| match n {
            Node::Element(k) if k.style.display == Some(Display::None) => true,
            // OOF не участвует в членстве линии и не создаёт разрыв потока.
            Node::Element(k) if out_of_flow(&k.style) => true,
            Node::Element(k) if !k.inline || flex => {
                monolith_blockified(k) || eligible(k, depth - 1, flex)
            }
            n => is_blank(n),
        })
    }
    fn contains(e: &Element, depth: u8) -> bool {
        flex_container(e) || (depth > 0 && e.children.iter().any(|n| {
            matches!(n, Node::Element(k) if !out_of_flow(&k.style) && contains(k, depth - 1))
        }))
    }
    contains(e, depth) && eligible(e, depth, false)
}
