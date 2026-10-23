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
        if e.style.cascaded().wm().vertical()
            || clone_dec(e).is_some()
            || e.style.used().webkit_box == Some(true)
            || crate::layout::multicol::spanner::multicol_container(&e.style.used())
            || !crate::layout::page::paged::visible_overflow(&e.style.used())
        {
            return false;
        }
        if grid_tree(e) || table_tree(e) || (!flex_container(e) && super::plain_tree(e)) {
            return true;
        }
        let flex = flex_container(e);
        if !(flex
            || matches!(
                e.style.used().display,
                None | Some(Display::Block | Display::ListItem)
            )
            || item && e.style.used().display == Some(Display::InlineBlock))
        {
            return false;
        }
        children(&e.children).all(|n| match n {
            // OOF не участвует в членстве линии и не создаёт разрыв потока.
            Node::Element(k) if out_of_flow(&k.style.used()) => true,
            // Атомарная строчная коробка блочного контейнера стоит в его
            // строке: разрыв — между строками, сама она неделима
            // (css-break-3 §4.1, §4.3 класс B).
            Node::Element(k) if !flex && k.style.used().display == Some(Display::InlineBlock) => {
                true
            }
            Node::Element(k) if !k.inline || flex => monolith_blockified(k) || eligible(k, flex),
            n => is_blank(n),
        })
    }
    fn contains(e: &Element) -> bool {
        flex_container(e)
            || children(&e.children).any(
                |n| matches!(n, Node::Element(k) if !out_of_flow(&k.style.used()) && contains(k)),
            )
    }
    contains(e) && eligible(e, false)
}
