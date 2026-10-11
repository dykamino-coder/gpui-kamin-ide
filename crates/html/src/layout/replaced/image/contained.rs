//! Картинка под обособлением размера (contain: size): размеры из contain-intrinsic-size.

use super::with::image_with;
use crate::dom::Element;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, StyledImage, px};

#[allow(clippy::result_large_err)]
pub(super) fn contained_image(
    base_font: Option<f32>,
    src: &str,
    e: &Element,
    d: gpui::Div,
) -> Result<AnyElement, gpui::Div> {
    if e.style.contains_width() || e.style.contains_height() {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = e.style.borders();
        let pad_x =
            side(e.style.padding.left) + side(e.style.padding.right) + side(b.left) + side(b.right);
        let pad_y =
            side(e.style.padding.top) + side(e.style.padding.bottom) + side(b.top) + side(b.bottom);
        let used = |explicit: Option<Len>, ci: Option<f32>| match explicit {
            Some(Len::Px(v)) => v,
            _ => ci.unwrap_or(0.0),
        };
        // Обособление снимает ПРИРОДНОЕ соотношение, но не ЗАЯВЛЕННОЕ:
        // css-contain-2 §size containment, «Size containment only suppresses
        // the natural aspect ratio, so properties like 'aspect-ratio' which
        // affect that preferred aspect ratio directly are honored»
        // (Overview.bs:659-662), и пример там же (:737-752):
        // `img{width:100px;aspect-ratio:1/1;contain:size}` = 100×100, а без
        // объявленного соотношения — 100×0. Пара атрибутов `width`/`height`
        // разметки — это `aspect-ratio: auto <ratio>` (HTML Rendering
        // §attributes for embedded content): природная его половина снята,
        // заявленная осталась. Blink делает ровно так —
        // `BlockNode::GetReplacedAspectRatio` (`block_node.cc:1328`):
        // заявленное отдаётся ДО проверки обособления, гейт
        // `ShouldApplyAnySizeContainment` стоит только вокруг природного.
        let ratio = e
            .style
            .aspect_ratio
            .filter(|r| r.is_finite() && *r > 0.0)
            .or_else(|| match (e.style.attr_width, e.style.attr_height) {
                (Some(Len::Px(aw)), Some(Len::Px(ah))) if aw > 0.0 && ah > 0.0 => Some(aw / ah),
                _ => None,
            });
        let named = |l: Option<Len>| matches!(l, Some(Len::Px(_)));
        let (nw, nh) = (named(e.style.width), named(e.style.height));
        let cw = match ratio {
            Some(r) if !nw && nh => used(e.style.height, e.style.contain_intrinsic.1) * r,
            _ => used(e.style.width, e.style.contain_intrinsic.0),
        };
        let ch = match ratio {
            Some(r) if nw && !nh => used(e.style.width, e.style.contain_intrinsic.0) / r,
            _ => used(e.style.height, e.style.contain_intrinsic.1),
        };
        let mut d = d;
        if e.style.contains_width() && matches!(e.style.width, None | Some(Len::Auto)) {
            d = d.w(px(cw + pad_x));
        }
        if e.style.contains_height() && matches!(e.style.height, None | Some(Len::Auto)) {
            d = d.h(px(ch + pad_y));
        }
        // Источник берётся тем же путём, что и вне обособления: строку со
        // схемой `file:` система уходит скачивать, и картинка молча не
        // рисуется. Растр декодируется своим декодером, вектор растрируется
        // в уже посчитанный размер.
        let local = src
            .strip_prefix("file:///")
            .or_else(|| src.strip_prefix("file://"))
            .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
        let mut image = match crate::paint::background::source(&crate::paint::background::key(
            local.unwrap_or(src),
            &e.style,
        )) {
            Some(crate::paint::background::Source::Vector { markup, .. })
                if cw > 0.0 && ch > 0.0 =>
            {
                match crate::svg::raster::rasterize(&markup, cw, ch) {
                    Some(r) => gpui::img(r),
                    None => gpui::img(SharedString::from(src.to_string())),
                }
            }
            Some(crate::paint::background::Source::Raster(ready)) => gpui::img(ready)
                .preserve_natural_pixels(true)
                .pixelated(e.style.image_pixelated == Some(true)),
            _ => match local {
                Some(path) => gpui::img(std::path::PathBuf::from(path)),
                None => gpui::img(SharedString::from(src.to_string())),
            },
        };
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
            return Ok(image_with(&fitted, base_font));
        }
        image = image.w(px(cw)).h(px(ch)).object_fit(gpui::ObjectFit::Fill);
        return Ok(d.child(image).into_any_element());
    }
    Err(d)
}
