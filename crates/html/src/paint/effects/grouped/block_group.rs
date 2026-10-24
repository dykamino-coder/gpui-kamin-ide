//! Групповые эффекты блочного бокса: стиль, который читает его обёртка, и
//! изоляция, нужная его blending-потомкам.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::computed::{BoxStyle, Computed};
use gpui::AnyElement;
use std::rc::Rc;

/// The box's own style, with mask image lengths and colors taken from its
/// inherited computed value; the wrapper keeps the box's own positioning
/// and clipping.
pub(crate) fn group_style(
    own: &crate::dom::SharedStyle,
    inherited: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Rc<Computed> {
    let style = own.used();
    if style.mask_image.is_none() {
        return style;
    }
    let computed = crate::style::cascade::materialize::box_style(inherited, own, vcx);
    let mut style = (*style).clone();
    style.mask_image = computed.mask_image.clone();
    Rc::new(style)
}

/// `grouped` around the built box `el` of block `e`.
///
/// css-compositing-1 §mix-blend-mode: a blending descendant blends only with
/// the content of ITS stacking context, so that context composites as a
/// separate group; otherwise the whole frame becomes the backdrop (a white
/// page around the parent gave a red ring in
/// `-blended-element-with-transparent-pixels`). Blink —
/// `PaintLayer::HasNonIsolatedDescendantWithBlendMode`.
pub(crate) fn block_group(
    el: AnyElement,
    e: &Element,
    inherited: &BoxStyle,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let style = group_style(&e.style, inherited, vcx);
    if crate::paint::stacking::stacking_context(&e.style.used())
        && e.style.used().isolate != Some(true)
        && crate::paint::stacking::blends_inside(&e.children, 0)
    {
        let mut iso = (*style).clone();
        iso.isolate = Some(true);
        return super::grouped(el, &iso, vcx);
    }
    super::grouped(el, &style, vcx)
}

/// The paint group of block `e` around `container`, a container of its
/// border box with its descendants: the same chain as a block in the flow
/// (`position_element`): the folded shift and opacity on the container,
/// then its transform, then the grouped effects.
pub(crate) fn paint_group(
    container: gpui::Div,
    e: &Element,
    inherited: &BoxStyle,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let el = crate::paint::apply::static_shift(container, &e.style.used());
    let el = crate::paint::apply::apply_opacity(el, &e.style.used(), vcx);
    let el = crate::paint::effects::transform::transformed(
        gpui::IntoElement::into_any_element(el),
        &e.style.used(),
        inherited,
        vcx,
    );
    block_group(el, e, inherited, vcx)
}
