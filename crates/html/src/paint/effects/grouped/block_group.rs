//! Group effects of a block box: the style its wrapper reads and the
//! isolation its blending descendants require.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::computed::Computed;
use gpui::AnyElement;
use std::borrow::Cow;

/// The box's own style, with mask image lengths and colors taken from its
/// inherited computed value; the wrapper keeps the box's own positioning
/// and clipping.
pub(crate) fn group_style<'a>(
    own: &'a Computed,
    inherited: &Computed,
    vcx: ValueCx<'_>,
) -> Cow<'a, Computed> {
    if own.mask_image.is_none() {
        return Cow::Borrowed(own);
    }
    let computed = crate::style::cascade::inherit::inherit(inherited, own, vcx);
    let mut style = own.clone();
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
    if crate::paint::stacking::stacking_context(&e.style)
        && e.style.isolate != Some(true)
        && crate::paint::stacking::blends_inside(&e.children, 0)
    {
        let mut iso = style.into_owned();
        iso.isolate = Some(true);
        return super::grouped(el, &iso, vcx);
    }
    super::grouped(el, &style, vcx)
}
