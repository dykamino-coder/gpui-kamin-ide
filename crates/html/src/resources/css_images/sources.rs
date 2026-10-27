//! Выбор вычисленного CSS-операнда через таблицу запросов документа.
use super::Source;
use crate::document::state::ValueCx;
use crate::resources::images::{
    svg_fragment,
    types::{ColourPolicy, DecodeOptions, DecodedImage, OrientationMode},
    vector,
};

pub(crate) fn source(src: &str, vcx: ValueCx<'_>) -> Option<Source> {
    let (src, orient) = src
        .strip_prefix("exif-none|")
        .map_or((src, true), |s| (s, false));
    load_url(src, orient, vcx)
}
pub(crate) fn computed_source(
    image: &crate::style::computed::ImageSource,
    orient: bool,
    vcx: ValueCx<'_>,
) -> Option<Source> {
    use crate::style::computed::ImageSource;
    match image {
        ImageSource::Url(url) => load_url(url, orient, vcx),
        ImageSource::Invalid => None,
        ImageSource::Fallback { url, color } => load_url(url, orient, vcx).or_else(|| {
            color.as_ref().map(|c| Source::Gradient {
                image: std::sync::Arc::new(ImageSource::Color(c.clone())),
            })
        }),
        ImageSource::Filter { image, program } => Some(Source::Filtered {
            image: Box::new(computed_source(image, orient, vcx)?),
            list: Box::new(program.clone()),
        }),
        _ => Some(Source::Gradient {
            image: std::sync::Arc::new(image.clone()),
        }),
    }
}
pub(crate) fn element_source(
    src: &str,
    style: &crate::style::computed::Computed,
    vcx: ValueCx<'_>,
) -> Option<Source> {
    let orient = style.image_orient_none != Some(true);
    match style.content_image() {
        Some(image) => computed_source(image, orient, vcx),
        None => load_url(src, orient, vcx),
    }
}
fn load_url(src: &str, orient: bool, vcx: ValueCx<'_>) -> Option<Source> {
    let options = DecodeOptions {
        orientation: if orient {
            OrientationMode::FromImage
        } else {
            OrientationMode::None
        },
        colour: ColourPolicy::Preserve,
    };
    let (image, fragment) = vcx.doc.images.borrow_mut().resolve_image(src, options)?;
    match image {
        DecodedImage::Raster(r) => Some(Source::Raster(super::presentation::raster(&r))),
        DecodedImage::Vector(v) => {
            let markup = if let Some(id) = fragment {
                String::from_utf8(svg_fragment::rewrite_svg_fragment(
                    v.markup.as_bytes().to_vec(),
                    &id,
                ))
                .ok()?
            } else {
                v.markup.to_string()
            };
            let (url, _) = vcx.doc.images.borrow().resolve(src);
            let markup =
                crate::resources::images::svg_links::with_base(&markup, url.base_url().as_ref());
            let size = vector::svg_size(&markup);
            Some(Source::Vector { markup, size })
        }
    }
}

#[cfg(test)]
mod tests {
    //! URL decoding takes precedence over a dimensionless image() color fallback.

    use super::{Source, computed_source};
    use crate::style::computed::ImageSource;
    fn parsed(raw: &str, vcx: crate::document::state::ValueCx<'_>) -> Option<Source> {
        computed_source(&ImageSource::parse(raw, vcx)?, true, vcx)
    }
    use crate::style::{computed::Computed, values::value::Color};

    #[test]
    fn decode_url_before_using_fallback() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let image = parsed("image('data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%222%22 height=%223%22/%3E', red)", vcx).unwrap();
        assert!(matches!(image, Source::Vector { .. }));
        let size = image.intrinsic();
        assert_eq!((size.w, size.h), (Some(2.0), Some(3.0)));
        for url in [
            "data:text/plain,not-an-image",
            "missing-image-fallback-unit.png",
        ] {
            let image = parsed(&format!("image('{url}', green)"), vcx).unwrap();
            assert!(matches!(image, Source::Gradient { .. }));
            let size = image.intrinsic();
            assert_eq!((size.w, size.h, size.ratio), (None, None, None));
            assert!(parsed(&format!("image('{url}')"), vcx).is_none());
        }
    }

    #[test]
    fn fallback_current_color_resolves_after_inheritance_with_alpha() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.color = Color::parse("rgba(0, 128, 0, 0.5)", vcx);
        let mut src =
            ImageSource::parse("image('data:text/plain,invalid', currentColor)", vcx).unwrap();
        src.resolve_colors(c.color.unwrap());
        let Some(Source::Gradient { image }) = computed_source(&src, true, vcx) else {
            panic!("fallback missing")
        };
        assert_eq!(
            match image.as_ref() {
                crate::style::computed::ImageSource::Color(c) => Some(c.value),
                _ => None,
            },
            c.color
        );
    }
}
