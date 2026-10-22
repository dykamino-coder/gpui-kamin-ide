//! Локальная картинка: источник, путь объектного вида (object-*), вписывание в коробку.

use super::content_radii::content_radii;
use super::piped::piped_image;
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::replaced::image::{Picture, PictureSizing};
use crate::layout::replaced::replaced_holder_ratio;
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
    image = match e.style.object_fit.as_deref() {
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
    let fills_holder =
        replaced_holder_ratio::apply(&mut d, &e.style, sizing.ratio(), узкая.is_some(), vcx);
    if fills_holder {
        image = image.size_full().object_fit(gpui::ObjectFit::Fill);
    }
    let image = position(image, &e.style, vcx);
    if let Some(source @ crate::paint::background::Source::Filtered { .. }) =
        crate::paint::background::element_source(local.unwrap_or(src), &e.style)
    {
        let mut style = (*e.style).clone();
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
            let mut image = image.w(gpui::relative(1.0));
            if let Some(r) = ratio {
                image.style().aspect_ratio = Some(r);
            }
            image
        }
        Picture::FullHeight { ratio } => {
            let mut image = image.h(gpui::relative(1.0));
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
    let local = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
    let own =
        crate::paint::background::element_source(local.unwrap_or(src), &e.style).and_then(|s| {
            match s {
                crate::paint::background::Source::Raster(image) => Some(image),
                filtered @ crate::paint::background::Source::Filtered { .. } => filtered.raster(
                    crate::paint::background::default_size(filtered.intrinsic(), (300.0, 150.0)),
                    vcx,
                ),
                crate::paint::background::Source::Vector { .. }
                | crate::paint::background::Source::Gradient { .. }
                | crate::paint::background::Source::Shape { .. } => None,
            }
        });
    let wants_pipe = e.style.object_position.is_some()
        || e.style
            .object_fit
            .as_deref()
            .is_some_and(|f| matches!(f, "fill" | "contain" | "cover" | "none" | "scale-down"));
    let mut d = d;
    if !wants_pipe
        && !matches!(
            e.style.content_image(),
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
    let mut image = match (own, local) {
        (Some(ready), _) => gpui::img(ready).preserve_natural_pixels(true),
        (None, Some(path)) => gpui::img(std::path::PathBuf::from(path)),
        (None, None) => gpui::img(SharedString::from(src.to_string())),
    };
    // css-images-3 §image-rendering: `pixelated` scales by nearest neighbour.
    image = image.pixelated(e.style.image_pixelated == Some(true));
    if e.style.filter.is_some_and(|f| f.grayscale > 0.5) {
        image = image.grayscale(true);
    }
    // Содержимое замещаемого режется кривой ВНУТРЕННЕГО края его поля
    // содержимого (css-backgrounds-3 §5.3: «the replaced element's content
    // is clipped to the curve of the content edge»): радиус угла минус
    // рамка и отбивка прилегающих сторон (`origin-*-box_with_radius-ref`:
    // `img { border-top-left-radius: 60px }`).
    let [tl, tr, br, bl] = content_radii(&e.style);
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
