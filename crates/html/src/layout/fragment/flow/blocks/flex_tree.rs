//! Поддеревья flex, чьи переносы выражаются измеренными коробками и ростом Regrow.
use super::{flex_container, grid_tree, table_tree};
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::{clone_dec, monolith_blockified};
use crate::style::computed::Display;

pub(crate) fn flex_tree(e: &Element) -> bool {
    fn eligible(e: &Element, item: bool) -> bool {
        if e.style.cascaded().wm().vertical()
            || clone_dec(e).is_some()
            || e.style.computed().webkit_box == Some(true)
            || crate::layout::multicol::spanner::multicol_container(&e.style.computed())
            || !crate::layout::page::paged::visible_overflow(&e.style.computed())
        {
            return false;
        }
        if grid_tree(e) || table_tree(e) || (!flex_container(e) && super::plain_tree(e)) {
            return true;
        }
        let flex = flex_container(e);
        if !(flex
            || matches!(
                e.style.computed().display,
                None | Some(Display::Block | Display::ListItem)
            )
            || item && e.style.computed().display == Some(Display::InlineBlock))
        {
            return false;
        }
        e.children.iter().all(|n| match n {
            Node::Element(k) if k.style.computed().display == Some(Display::None) => true,
            // OOF не участвует в членстве линии и не создаёт разрыв потока.
            Node::Element(k) if out_of_flow(&k.style.computed()) => true,
            // Атомарная строчная коробка блочного контейнера стоит в его
            // строке: разрыв — между строками, сама она неделима
            // (css-break-3 §4.1, §4.3 класс B).
            Node::Element(k)
                if !flex && k.style.computed().display == Some(Display::InlineBlock) =>
            {
                true
            }
            Node::Element(k) if !k.inline || flex => monolith_blockified(k) || eligible(k, flex),
            n => is_blank(n),
        })
    }
    fn contains(e: &Element) -> bool {
        flex_container(e)
            || e.children
                .iter()
                .any(|n| matches!(n, Node::Element(k) if !out_of_flow(&k.style.computed()) && contains(k)))
    }
    contains(e) && eligible(e, false)
}
