//! Картинка под обособлением размера (contain: size): рисунок в размере из
//! `contain-intrinsic-size` (`contained_size`).

use super::with::image_with;
use crate::dom::Element;
use crate::layout::replaced::image::contained_size::{ContainedSize, contained_size};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, StyledImage, px};

#[allow(clippy::result_large_err)]
pub(super) fn contained_image(
    base_font: Option<f32>,
    src: &str,
    e: &Element,
    d: gpui::Div,
    cx: &crate::layout::cx::LayoutCx,
) -> Result<AnyElement, gpui::Div> {
    if let Some(ContainedSize {
        content: (cw, ch),
        holder: (holder_w, holder_h),
    }) = contained_size(&e.style)
    {
        let mut d = d;
        if let Some(w) = holder_w {
            d = d.w(px(w));
        }
        if let Some(h) = holder_h {
            d = d.h(px(h));
        }
        let vcx = cx.vcx();
        let ready = crate::resources::css_images::element_source(src, &e.style, vcx)
            .map(crate::paint::background::Source::from_cpu)
            .and_then(|s| match s {
                crate::paint::background::Source::Vector { markup, .. } => {
                    let viewport =
                        crate::resources::images::viewport::with_viewport(&markup, (cw, ch));
                    crate::svg::raster::rasterize_image(&viewport, cw, ch, vcx)
                }
                _ => s.raster((cw, ch), vcx),
            });
        let Some(ready) = ready else {
            return Ok(d.into_any_element());
        };
        let mut image = gpui::img(ready)
            .preserve_natural_pixels(true)
            .pixelated(e.style.image_pixelated == Some(true));
        // Подгонка содержимого замещаемой коробки считается от ПРИРОДНОГО
        // размера картинки, который у самой картинки никуда не делся:
        // обособление меняет коробку, а не объект. У Blink это два разных
        // входа — раскладочный `LayoutReplaced::ComputeNaturalSizingInfo`
        // начинается с `DCHECK(!ShouldApplySizeContainment())`
        // (`layout_replaced.cc:509`), а рисовательный
        // `LayoutReplaced::ReplacedContentRectFrom` берёт
        // `GetNaturalDimensions()` БЕЗ всякого гейта (`:497`), и уже от него
        // `ComputeObjectFitAndPositionRect` (`:426`) считает `object-fit`.
        // Поэтому коробку домеряем здесь, а рисуем ОБЫЧНЫМ путём: клон с уже
        // посчитанными сторонами содержимого и снятым обособлением — это и
        // есть второй такт, «laying out in-place» (css-contain-2
        // Overview.bs:702-707).
        if e.style.object_fit.is_some() || e.style.object_position.is_some() {
            let mut fitted = e.clone();
            fitted.style.width = Some(Len::Px(cw));
            fitted.style.height = Some(Len::Px(ch));
            // `cw`/`ch` — размер СОДЕРЖИМОГО по построению, поэтому клон
            // меряется по содержимому независимо от `box-sizing` документа.
            fitted.style.border_box = Some(false);
            fitted.style.contain_size = Some(false);
            fitted.style.contain_inline_size = Some(false);
            return Ok(image_with(&fitted, base_font, cx));
        }
        image = image.w(px(cw)).h(px(ch)).object_fit(gpui::ObjectFit::Fill);
        return Ok(d.child(image).into_any_element());
    }
    Err(d)
}
