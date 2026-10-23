//! Атом строчного потока для `FlowRow` в статическом и измеряемом хостах обтекания:
//! инлайн-блок или `<img>` с margin-box в точках строится элементом размером с margin-box,
//! поля кладёт слот-обёртка, а содержимое рисует border-box один раз (CSS 2.1 §9.4.2,
//! §10.6.6). Позиционированные потомки получают ключ содержащего блока DOM.

use crate::dom::Element;
use crate::layout::float::band_host::{px_margin, px_margin_box};
use crate::layout::options::RenderOpts;
use crate::layout::replaced::limits::{atom_base_font, with_inherited_font};
use crate::style::cascade::materialize::box_style;
use crate::style::computed::Computed;
use gpui::{IntoElement, ParentElement, Styled, div, px};

/// Атом строчного потока для `FlowRow`: инлайн-блок с margin-box в
/// точках. Поля кладёт слот (обёртка), а не сама коробка.
pub(super) fn band_atom(
    c: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<crate::layout::fragment::types::FlowChild> {
    let vcx = opts.cx.vcx();
    let (w, h) = px_margin_box(&c.style.computed())?;
    let (ml, mt) = (
        px_margin(&c.style.computed().margin.left).unwrap_or(0.0),
        px_margin(&c.style.computed().margin.top).unwrap_or(0.0),
    );
    let mut merged = box_style(inherited, &c.style, vcx);
    // This path bypasses element(); positioned descendants still need the DOM CB key.
    merged.self_node = c.node_id;
    // CSS 2.1 sections 9.4.2 and 10.6.6: the outer holder already
    // represents the margin box; its contents paint the border box once.
    merged.margin = crate::style::computed::Sides::default();
    let mut inner = c.clone();
    inner.style.used_mut().margin = crate::style::computed::Sides::default();
    let built = if inner.tag == "img" {
        opts.cx.build.replaced(
            &with_inherited_font(&inner, inherited),
            Some(atom_base_font(inherited, opts)),
            &opts.cx,
        )
    } else {
        opts.cx
            .build
            .styled_box(&inner, &merged, &opts.cx)
            .children(opts.cx.build.blocks(&inner.children, &merged, opts))
            .into_any_element()
    };
    let el = if px_margin_box(&inner.style.computed()) == Some((w, h)) {
        built
    } else {
        div()
            .relative()
            .w(px(w))
            .h(px(h))
            .child(div().absolute().left(px(ml)).top(px(mt)).child(built))
            .into_any_element()
    };
    Some(crate::layout::fragment::types::FlowChild { el, w, h })
}
