//! Групповые эффекты блочного бокса: стиль, который читает его обёртка, и
//! изоляция, нужная его blending-потомкам.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::computed::Computed;
use gpui::AnyElement;
use std::borrow::Cow;

/// The box's own style, with mask image lengths and colors taken from its
/// inherited computed value; the wrapper keeps the box's own positioning
/// and clipping.
pub(crate) fn group_style<'a>(
    own: &'a crate::dom::SharedStyle,
    inherited: &Computed,
    vcx: ValueCx<'_>,
) -> Cow<'a, Computed> {
    if own.cascaded().mask_image.is_none() {
        return Cow::Borrowed(own.cascaded());
    }
    let computed = crate::style::cascade::materialize::box_style(inherited, own, vcx);
    let mut style = own.cascaded().clone();
    style.mask_image = computed.mask_image;
    Cow::Owned(style)
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
    inherited: &Computed,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let style = group_style(&e.style, inherited, vcx);
    if crate::paint::stacking::stacking_context(e.style.cascaded())
        && e.style.cascaded().isolate != Some(true)
        && crate::paint::stacking::blends_inside(&e.children, 0)
    {
        let mut iso = style.into_owned();
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
    inherited: &Computed,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let el = crate::paint::apply::static_shift(container, e.style.cascaded());
    let el = crate::paint::apply::apply_opacity(el, e.style.cascaded(), vcx);
    let el = crate::paint::effects::transform::transformed(
        gpui::IntoElement::into_any_element(el),
        e.style.cascaded(),
        inherited,
        vcx,
    );
    block_group(el, e, inherited, vcx)
}
