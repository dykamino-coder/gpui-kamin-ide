//! Локальная картинка: источник, путь объектного вида (object-*), вписывание в коробку.

use super::sized_image;
use crate::dom::Element;
use crate::layout::replaced::{replaced_content, replaced_holder_ratio};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, StyledImage, div, px};

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
    if replaced_holder_ratio::apply(&mut d, &e.style, ratio_of(), узкая.is_some()) {
        image = image.size_full().object_fit(gpui::ObjectFit::Fill);
    }
    d.child(replaced_content::position(image, &e.style))
        .into_any_element()
    // CSS Images 3 §4.5: object-fit initially fills the content box.
    // Intrinsic sizing above already preserves the natural ratio. Applying
    // contain again to the snapped box introduces unintended letterboxing.
}

pub(super) fn local_image_source<'a>(
    src: &'a str,
    e: &Element,
    d: gpui::Div,
) -> Result<(Option<&'a str>, gpui::Div, gpui::Img), AnyElement> {
    let local = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
    let key = crate::paint::background::key(local.unwrap_or(src), &e.style);
    let own = crate::paint::background::source(&key).and_then(|s| match s {
        crate::paint::background::Source::Raster(image) => Some(image),
        crate::paint::background::Source::Vector { .. }
        | crate::paint::background::Source::Gradient { .. }
        | crate::paint::background::Source::Shape { .. } => None,
    });
    let wants_pipe = e.style.object_position.is_some()
        || e.style
            .object_fit
            .as_deref()
            .is_some_and(|f| matches!(f, "fill" | "contain" | "cover" | "none" | "scale-down"));
    let d = match piped_image(src, e, d, local, wants_pipe) {
        Ok(value) => return Err(value),
        Err(d) => d,
    };
    let mut image = match (own, local) {
        (Some(ready), _) => gpui::img(ready).preserve_natural_pixels(true),
        (None, Some(path)) => gpui::img(std::path::PathBuf::from(path)),
        (None, None) => gpui::img(SharedString::from(src.to_string())),
    };
    if e.style.filter.is_some_and(|f| f.grayscale > 0.5) {
        image = image.grayscale(true);
    }
    Ok((local, d, image))
}

#[allow(clippy::result_large_err)]
pub(super) fn piped_image(
    src: &str,
    e: &Element,
    d: gpui::Div,
    local: Option<&str>,
    wants_pipe: bool,
) -> Result<AnyElement, gpui::Div> {
    if let (true, Some(Len::Px(w)), Some(Len::Px(h))) = (wants_pipe, e.style.width, e.style.height)
    {
        let pos = e
            .style
            .object_position
            .unwrap_or(crate::style::computed::BgPos {
                x: Some(Len::Pct(0.5)),
                y: Some(Len::Pct(0.5)),
            });
        use crate::style::computed::BgSize;
        let mut bgc = crate::style::computed::Computed::default();
        bgc.bg_image = Some(local.unwrap_or(src).to_string());
        bgc.bg_repeat = Some(crate::style::computed::BgRepeat::NoRepeat);
        bgc.bg_pos = pos;
        // Стиль трубы собран с нуля, и отказ от EXIF-разворота в него надо
        // положить руками: иначе `image-orientation: none` вместе с
        // `object-fit`/`object-position` уходил бы мимо ключа источника.
        bgc.image_orient_none = e.style.image_orient_none;
        bgc.bg_size = match e.style.object_fit.as_deref() {
            Some("contain") => BgSize::Contain,
            Some("cover") => BgSize::Cover,
            Some("none") => BgSize::Auto,
            Some("scale-down") => {
                // Меньшее из `none` и `contain`: влезает — своим
                // размером, нет — вписать.
                let fits = crate::paint::background::source(&crate::paint::background::key(
                    local.unwrap_or(src),
                    &e.style,
                ))
                .map(|s| s.intrinsic())
                .is_some_and(|i| i.w.is_some_and(|iw| iw <= w) && i.h.is_some_and(|ih| ih <= h));
                if fits { BgSize::Auto } else { BgSize::Contain }
            }
            _ => BgSize::Fixed(Some(Len::Pct(1.0)), Some(Len::Pct(1.0))),
        };
        // `overflow: visible` на замещаемом (HTML §rendering: UA-правило
        // `img { overflow: clip; overflow-clip-margin: content-box }`,
        // css-overflow-3): ЯВНОЕ `visible` выпускает картинку за content
        // box — `object-fit: none` рисуется своим размером целиком,
        // скругление её тоже не режет (`overflow-img`, `-svg`,
        // `-border-radius`: эталон — та же картинка без обрезки).
        // Умолчание (`None`) — UA-шный `clip`, прежний путь ниже.
        let spills = e.style.overflow_x == Some(crate::style::computed::Overflow::Visible)
            && e.style.overflow_y == Some(crate::style::computed::Overflow::Visible);
        if spills {
            let style = bgc.clone();
            let layer = gpui::canvas(
                |_, _, _| {},
                move |bounds: gpui::Bounds<gpui::Pixels>, _, window, _| {
                    // Область ОТСЧЁТА — content box, область КРАСКИ — с
                    // запасом во все стороны: плитка одна (`no-repeat`),
                    // и рисуется она ровно своим размером.
                    let reach = px(4096.0);
                    let paint = gpui::Bounds {
                        origin: gpui::point(bounds.origin.x - reach, bounds.origin.y - reach),
                        size: gpui::size(
                            bounds.size.width + reach * 2.0,
                            bounds.size.height + reach * 2.0,
                        ),
                    };
                    crate::paint::background::paint_tiles(&style, bounds, Some(paint), window);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full();
            return Ok(d
                .child(div().w(px(w)).h(px(h)).relative().child(layer))
                .into_any_element());
        }
        if let Some(layer) = crate::paint::background::layer(&bgc) {
            // Внутренняя коробка = content box: поля и рамка остаются
            // на хосте, слой не должен их накрывать.
            return Ok(d
                .child(
                    div()
                        .w(px(w))
                        .h(px(h))
                        .relative()
                        .overflow_hidden()
                        .child(layer),
                )
                .into_any_element());
        }
    }
    Err(d)
}
