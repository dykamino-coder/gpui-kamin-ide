//! Anonymous inline runs inside list containers participate in normal block flow.
//! CSS 2.1 §9.2.1.1 groups consecutive inline content between block children.

pub(crate) mod list_item;

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::render::{blocks, styled_div_with};
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, IntoElement, ParentElement, Styled};

pub(crate) fn render(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let mut rows = Vec::new();
    let mut pending = Vec::new();
    for child in &e.children {
        match child {
            Node::Element(li) if li.tag == "li" && !crate::layout::box_kind::real_inline(li) => {
                rows.extend(blocks(&std::mem::take(&mut pending), inherited, opts));
                rows.push(list_item::render(li, inherited, opts));
            }
            Node::Element(pseudo)
                if pseudo.tag.starts_with("::")
                    && pseudo.style.display != Some(Display::ListItem) => {}
            _ => pending.push(child.clone()),
        }
    }
    rows.extend(blocks(&pending, inherited, opts));
    styled_div_with(e, inherited, &opts.cx)
        .flex()
        .flex_col()
        .children(rows)
        .into_any_element()
}
