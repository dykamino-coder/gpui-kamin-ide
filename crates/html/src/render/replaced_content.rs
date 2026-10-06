//! Replaced content can be empty without erasing its surrounding CSS box.
use super::styled_div_with;
use crate::computed::Computed;
use crate::dom::Element;
use crate::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, StyledImage, px};

pub(super) fn position(mut image: gpui::Img, style: &Computed) -> gpui::Img {
    if let Some(position) = style.object_position {
        image = image.object_position(gpui::point(
            crate::apply::len_to_gpui(position.x.unwrap_or(Len::Pct(0.5))),
            crate::apply::len_to_gpui(position.y.unwrap_or(Len::Pct(0.5))),
        ));
    }
    image
}

/// Замещаемый `<svg>` с учётом его CSS-коробки (CSS 2.1 §10.3.4): растр —
/// содержимое, а при ненулевой рамке или отбивке (`svg_has_box`) — внутри
/// стилевого `div` размером border-box, фон красит он. Прежде растр шёл
/// голым и рамка не рисовалась вовсе (border-shape-clips-background-ref,
/// mask-image-svg-child-will-change: маска ложится на коробку 200×200 с
/// рамкой 50). Без рамки и отбивки — голый растр, путь прежний.
/// `None` — рисунок не разобрался.
pub(super) fn svg_replaced(e: &Element, sized: &Element, merged: &Computed) -> Option<AnyElement> {
    let boxed = svg_has_box(merged);
    let inner;
    let sized = if boxed {
        let mut copy = sized.clone();
        copy.style.background = None;
        inner = copy;
        &inner
    } else {
        sized
    };
    let raster = crate::svg::element(sized);
    if !boxed {
        return raster;
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let (w, h) = crate::svg::size_of(sized);
    // Empty replaced content still has its CSS padding, border and background.
    if raster.is_none() && w > 0.0 && h > 0.0 {
        return None;
    }
    let b = merged.borders();
    let bw = w
        + px_of(b.left)
        + px_of(b.right)
        + px_of(merged.padding.left)
        + px_of(merged.padding.right);
    let bh = h
        + px_of(b.top)
        + px_of(b.bottom)
        + px_of(merged.padding.top)
        + px_of(merged.padding.bottom);
    Some(
        styled_div_with(e, merged)
            .w(px(bw))
            .h(px(bh))
            .flex_shrink_0()
            .children(raster)
            .into_any_element(),
    )
}

/// У `<svg>` есть своя CSS-коробка — ненулевая рамка или отбивка
/// (см. ветку `"svg"` в `element`): тогда растр кладётся в стилевой `div`.
pub(super) fn svg_has_box(c: &Computed) -> bool {
    let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v > 0.0);
    let b = c.borders();
    nz(b.top)
        || nz(b.right)
        || nz(b.bottom)
        || nz(b.left)
        || nz(c.padding.top)
        || nz(c.padding.right)
        || nz(c.padding.bottom)
        || nz(c.padding.left)
}
