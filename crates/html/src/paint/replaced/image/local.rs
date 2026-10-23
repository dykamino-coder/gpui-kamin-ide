//! Локальная картинка: источник, путь объектного вида (object-*), вписывание в коробку.

use super::content_radii::content_radii;
use super::holder_ratio;
use super::piped::piped_image;
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::replaced::image::{Picture, PictureSizing};
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, StyledImage, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn fit_local_image(
    src: &str,
    e: &Element,
    local: Option<&str>,
    d: gpui::Div,
    image: gpui::Img,
    vectorize: impl Fn(gpui::Img, f32, f32) -> gpui::Img,
    sizing: &PictureSizing<'_>,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let (picture, узкая) = sizing.picture();
    let mut image = draw(image, picture, vectorize);
    image = match e.style.computed().object_fit.as_deref() {
        Some("cover") => image.object_fit(gpui::ObjectFit::Cover),
        Some("contain") => image.object_fit(gpui::ObjectFit::Contain),
        Some("scale-down") => image.object_fit(gpui::ObjectFit::ScaleDown),
        Some("none") => image.object_fit(gpui::ObjectFit::None),
        _ => image.object_fit(gpui::ObjectFit::Fill),
    };
    let (sub_w, sub_h) = sizing.sub;
    let mut d = match узкая {
        Some((w, h)) => d.w(px(w + sub_w)).h(px(h + sub_h)),
        None => d,
    };
    let fills_holder = holder_ratio::apply(
        &mut d,
        &BoxStyle::shared(e.style.computed()),
        sizing.ratio(),
        узкая.is_some(),
        vcx,
    );
    if fills_holder {
        image = image.size_full().object_fit(gpui::ObjectFit::Fill);
    }
    let image = position(image, &e.style.computed(), vcx);
    if let Some(source @ crate::paint::background::Source::Filtered { .. }) =
        crate::resources::css_images::element_source(local.unwrap_or(src), &e.style.computed(), vcx)
            .map(crate::paint::background::Source::from_cpu)
    {
        let mut style = (*e.style.computed()).clone();
        if fills_holder {
            style.object_fit = Some("fill".into());
        }
        return d
            .child(super::filtered::FilteredImage::new(
                image, source, style, vcx,
            ))
            .into_any_element();
    }
    d.child(image).into_any_element()
    // CSS Images 3 §4.5: object-fit initially fills the content box.
    // Intrinsic sizing above already preserves the natural ratio. Applying
    // contain again to the snapped box introduces unintended letterboxing.
}

/// Рисунок в размерах `picture`; вектор растрируется в размер растра.
fn draw(
    image: gpui::Img,
    picture: Picture,
    vectorize: impl Fn(gpui::Img, f32, f32) -> gpui::Img,
) -> gpui::Img {
    match picture {
        Picture::Natural => image,
        Picture::Authored {
            raster: (rw, rh),
            size,
        } => {
            let image = vectorize(image, rw, rh);
            match size {
                Some((w, h)) => image.w(px(w)).h(px(h)),
                None => image.size_full(),
            }
        }
        Picture::Used {
            raster: (rw, rh),
            size: (w, h),
        } => vectorize(image, rw, rh)
            .w(px(w))
            .h(px(h))
            .object_fit(gpui::ObjectFit::Fill),
        Picture::FullWidth { ratio } => {
            let mut image = image.w(gpui::relative(1.0)).h_auto();
            if let Some(r) = ratio {
                image.style().aspect_ratio = Some(r);
            }
            image
        }
        Picture::FullHeight { ratio } => {
            let mut image = image.h(gpui::relative(1.0)).w_auto();
            if let Some(r) = ratio {
                image.style().aspect_ratio = Some(r);
            }
            image
        }
        Picture::FullWidthPxHeight(h) => image.w(gpui::relative(1.0)).h(px(h)),
        Picture::PxWidthFullHeight(w) => image.w(px(w)).h(gpui::relative(1.0)),
        Picture::Full => image.size_full(),
    }
}

pub(super) fn local_image_source<'a>(
    src: &'a str,
    e: &Element,
    d: gpui::Div,
    vcx: ValueCx<'_>,
) -> Result<(Option<&'a str>, gpui::Div, gpui::Img), AnyElement> {
    let local = (src.starts_with("file:") || src.starts_with('/')).then_some(src);
    let own = crate::resources::css_images::element_source(src, &e.style.computed(), vcx)
        .map(crate::paint::background::Source::from_cpu)
        .and_then(|s| {
            let size = crate::paint::background::default_size(s.intrinsic(), (300.0, 150.0));
            let raster = match &s {
                crate::paint::background::Source::Vector { markup, .. } => {
                    crate::svg::raster::rasterize_image(
                        &crate::resources::images::viewport::with_viewport(markup, size),
                        size.0,
                        size.1,
                        vcx,
                    )
                }
                _ => s.raster(size, vcx),
            }?;
            Some((raster, size))
        });
    let wants_pipe = e.style.computed().object_position.is_some()
        || e.style
            .computed()
            .object_fit
            .as_deref()
            .is_some_and(|f| matches!(f, "fill" | "contain" | "cover" | "none" | "scale-down"));
    let mut d = d;
    if !wants_pipe
        && !matches!(
            e.style.computed().content_image(),
            Some(crate::style::computed::ImageSource::Filter { .. })
        )
        && let Some(image) = super::grid_measured::measured(e, local.unwrap_or(src), &mut d, vcx)
    {
        return Err(image);
    }
    let d = match piped_image(src, e, d, local, wants_pipe, vcx) {
        Ok(value) => return Err(value),
        Err(d) => d,
    };
    let Some((ready, size)) = own else {
        return Err(d
            .child(SharedString::from(
                e.attr("alt").unwrap_or("[изображение]").to_string(),
            ))
            .into_any_element());
    };
    let mut image = gpui::img(ready)
        .w(px(size.0))
        .h(px(size.1))
        .preserve_natural_pixels(true);
    // css-images-3 §image-rendering: `pixelated` scales by nearest neighbour.
    image = image.pixelated(e.style.cascaded().image_pixelated == Some(true));
    if e.style.computed().filter.is_some_and(|f| f.grayscale > 0.5) {
        image = image.grayscale(true);
    }
    // Содержимое замещаемого режется кривой ВНУТРЕННЕГО края его поля
    // содержимого (css-backgrounds-3 §5.3: «the replaced element's content
    // is clipped to the curve of the content edge»): радиус угла минус
    // рамка и отбивка прилегающих сторон (`origin-*-box_with_radius-ref`:
    // `img { border-top-left-radius: 60px }`).
    let [tl, tr, br, bl] = content_radii(&e.style.computed());
    if tl > 0.0 {
        image = image.rounded_tl(px(tl));
    }
    if tr > 0.0 {
        image = image.rounded_tr(px(tr));
    }
    if br > 0.0 {
        image = image.rounded_br(px(br));
    }
    if bl > 0.0 {
        image = image.rounded_bl(px(bl));
    }
    Ok((local, d, image))
}

fn position(
    mut image: gpui::Img,
    style: &crate::style::computed::Computed,
    vcx: ValueCx<'_>,
) -> gpui::Img {
    if let Some(position) = style.object_position {
        image = image.object_position(gpui::point(
            crate::paint::apply::len_to_gpui(position.x.unwrap_or(Len::Pct(0.5)), vcx),
            crate::paint::apply::len_to_gpui(position.y.unwrap_or(Len::Pct(0.5)), vcx),
        ));
    }
    image
}
