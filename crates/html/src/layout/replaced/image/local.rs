//! Локальная картинка: источник, путь объектного вида (object-*), вписывание в коробку.

use super::piped::piped_image;
use super::{content_radii::content_radii, sized_image};
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::replaced::{replaced_content, replaced_holder_ratio};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, StyledImage, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn fit_local_image(
    src: &str,
    e: &Element,
    local: Option<&str>,
    d: gpui::Div,
    mut image: gpui::Img,
    vectorize: impl Fn(gpui::Img, f32, f32) -> gpui::Img,
    sub_w: f32,
    sub_h: f32,
    clamp: impl Fn(Option<Len>, f32) -> Option<f32>,
    max_w: Option<f32>,
    max_h: Option<f32>,
    limit: impl Fn(f32, Option<f32>, Option<f32>) -> f32,
    mut узкая: Option<(f32, f32)>,
    ratio_of: impl Fn() -> Option<f32>,
    transfer: impl Fn(f32, f32, bool) -> f32,
    cb_width: Option<f32>,
    vcx: ValueCx<'_>,
) -> AnyElement {
    image = sized_image(
        src,
        e,
        local,
        image,
        vectorize,
        sub_w,
        sub_h,
        clamp,
        max_w,
        max_h,
        limit,
        &mut узкая,
        &ratio_of,
        transfer,
        cb_width,
        vcx,
    );
    image = match e.style.object_fit.as_deref() {
        Some("cover") => image.object_fit(gpui::ObjectFit::Cover),
        Some("contain") => image.object_fit(gpui::ObjectFit::Contain),
        Some("scale-down") => image.object_fit(gpui::ObjectFit::ScaleDown),
        Some("none") => image.object_fit(gpui::ObjectFit::None),
        _ => image.object_fit(gpui::ObjectFit::Fill),
    };
    let mut d = match узкая {
        Some((w, h)) => d.w(px(w + sub_w)).h(px(h + sub_h)),
        None => d,
    };
    let fills_holder =
        replaced_holder_ratio::apply(&mut d, &e.style, ratio_of(), узкая.is_some(), vcx);
    if fills_holder {
        image = image.size_full().object_fit(gpui::ObjectFit::Fill);
    }
    let image = replaced_content::position(image, &e.style, vcx);
    if let Some(source @ crate::paint::background::Source::Filtered { .. }) =
        crate::paint::background::source(
            &crate::paint::background::key(local.unwrap_or(src), &e.style, vcx),
            vcx,
        )
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

pub(super) fn local_image_source<'a>(
    src: &'a str,
    e: &Element,
    d: gpui::Div,
    vcx: ValueCx<'_>,
) -> Result<(Option<&'a str>, gpui::Div, gpui::Img), AnyElement> {
    let local = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .or_else(|| crate::paint::background::filter_image::parse(src, vcx).map(|_| src))
        .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
    let key = crate::paint::background::key(local.unwrap_or(src), &e.style, vcx);
    let own = crate::paint::background::source(&key, vcx).and_then(|s| match s {
        crate::paint::background::Source::Raster(image) => Some(image),
        filtered @ crate::paint::background::Source::Filtered { .. } => filtered.raster(
            crate::paint::background::default_size(filtered.intrinsic(), (300.0, 150.0)),
            vcx,
        ),
        crate::paint::background::Source::Vector { .. }
        | crate::paint::background::Source::Gradient { .. }
        | crate::paint::background::Source::Shape { .. } => None,
    });
    let wants_pipe = e.style.object_position.is_some()
        || e.style
            .object_fit
            .as_deref()
            .is_some_and(|f| matches!(f, "fill" | "contain" | "cover" | "none" | "scale-down"));
    let mut d = d;
    if !wants_pipe
        && crate::paint::background::filter_image::parse(src, vcx).is_none()
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
