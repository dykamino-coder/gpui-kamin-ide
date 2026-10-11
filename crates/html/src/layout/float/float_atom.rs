//! Build atomic inline margin boxes for measured and static float wrapping.

use crate::dom::Element;
use crate::layout::float::band_host::{px_margin, px_margin_box};
use crate::layout::replaced::image::image_with;
use crate::layout::replaced::limits::{atom_base_font, with_inherited_font};
use crate::render::{RenderOpts, blocks, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use gpui::{IntoElement, ParentElement, Styled, div, px};

/// Атом строчного потока для `FlowRow`: инлайн-блок с margin-box в
/// точках. Поля кладёт слот (обёртка), а не сама коробка — как у
/// статического хоста (`shape_flow`, ветка атомов).
pub(super) fn band_atom(
    c: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<crate::layout::fragment::types::FlowChild> {
    let (w, h) = px_margin_box(&c.style)?;
    let (ml, mt) = (
        px_margin(&c.style.margin.left).unwrap_or(0.0),
        px_margin(&c.style.margin.top).unwrap_or(0.0),
    );
    let mut merged = inherit(inherited, &c.style);
    // CSS 2.1 sections 9.4.2 and 10.6.6: the outer holder already
    // represents the margin box; its contents paint the border box once.
    merged.margin = crate::style::computed::Sides::default();
    let mut inner = c.clone();
    inner.style.margin = crate::style::computed::Sides::default();
    let built = if inner.tag == "img" {
        image_with(
            &with_inherited_font(&inner, inherited),
            Some(atom_base_font(inherited, opts)),
        )
    } else {
        styled_div_with(&inner, &merged)
            .children(blocks(&inner.children, &merged, opts))
            .into_any_element()
    };
    let el = if px_margin_box(&inner.style) == Some((w, h)) {
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
