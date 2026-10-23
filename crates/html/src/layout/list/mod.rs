//! Контейнер списка: дети `li` раскладываются как пункты (`list_item`), а подряд идущее
//! прочее содержимое между ними собирается в анонимные блоки строк (CSS 2.1 §9.2.1.1) и
//! идёт обычным блочным потоком. Псевдоэлементы, кроме `display: list-item`, пропускаются.

pub(crate) mod list_item;

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, IntoElement, ParentElement, Styled};

pub(crate) fn render(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let mut rows = Vec::new();
    let mut pending = Vec::new();
    for child in &e.children {
        match child {
            Node::Element(li) if li.tag == "li" && !crate::layout::box_kind::real_inline(li) => {
                rows.extend(
                    opts.cx
                        .build
                        .blocks(&std::mem::take(&mut pending), inherited, opts),
                );
                rows.push(list_item::render(li, inherited, opts));
            }
            Node::Element(pseudo)
                if pseudo.tag.starts_with("::")
                    && pseudo.style.computed().display != Some(Display::ListItem) => {}
            _ => pending.push(child.clone()),
        }
    }
    rows.extend(opts.cx.build.blocks(&pending, inherited, opts));
    opts.cx
        .build
        .styled_box(e, inherited, &opts.cx)
        .flex()
        .flex_col()
        .children(rows)
        .into_any_element()
}
