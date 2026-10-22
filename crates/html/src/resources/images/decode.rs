//! Декодер картинки из байт в нейтральное значение: рисунок SVG распознаётся по
//! содержимому (у `data:` расширения нет), растр — кодеками крейта `image` (тот же набор
//! форматов, что у GPUI). Порядок для растра: точки → цветовой профиль → EXIF-ориентация,
//! чтобы природный размер считался по развёрнутой картинке (css-images-3 §5.4).

use super::colour::{convert_to_srgb, png_icc_profile};
use super::orientation::{exif_orientation, orient_pixels};
use super::types::{
    AlphaMode, ColourPolicy, ColourSpace, CpuRaster, DecodeOptions, DecodedImage, ImageError,
    ImageMetadata, OrientationMode, PixelFormat,
};
use super::vector::{VectorImage, looks_svg, svg_size};
use crate::color::IccProfile;
use std::sync::Arc;

/// Декодировать байты картинки с данными параметрами.
pub(crate) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
) -> Result<(DecodedImage, ImageMetadata), ImageError> {
    if bytes.is_empty() {
        return Err(ImageError::InvalidData);
    }
    if looks_svg(bytes) {
        return decode_svg(bytes);
    }
    let mut rgba = image::load_from_memory(bytes)
        .map_err(codec_error)?
        .into_rgba8();
    let (mut width, mut height) = rgba.dimensions();
    let mut colour_space = ColourSpace::Srgb;
    if let Some(profile) = png_icc_profile(bytes) {
        match (options.colour, IccProfile::parse(&profile)) {
            (ColourPolicy::ConvertToSrgb, Some(parsed)) => convert_to_srgb(&mut rgba, &parsed),
            // Непонятый или сохранённый профиль остаётся описанием точек, а не молча sRGB.
            _ => colour_space = ColourSpace::Embedded(profile.into()),
        }
    }
    let mut pixels = rgba.into_raw();
    let mut applied_orientation = None;
    if options.orientation == OrientationMode::FromImage
        && let Some(tag @ 2..=8) = exif_orientation(bytes)
        && let Some((w, h, turned)) = orient_pixels(width, height, &pixels, tag)
    {
        (width, height, pixels) = (w, h, turned);
        applied_orientation = Some(tag);
    }
    let raster = CpuRaster {
        width,
        height,
        stride: width as usize * 4,
        format: PixelFormat::Rgba8,
        alpha: AlphaMode::Straight,
        colour_space,
        bytes: pixels.into(),
    };
    let image = DecodedImage::Raster(Arc::new(raster));
    let metadata = ImageMetadata {
        natural: image.natural(),
        applied_orientation,
        vector: false,
    };
    Ok((image, metadata))
}

fn decode_svg(bytes: &[u8]) -> Result<(DecodedImage, ImageMetadata), ImageError> {
    let markup = std::str::from_utf8(bytes).map_err(|_| ImageError::InvalidData)?;
    // Нулевой viewBox отключает отрисовку, но SVG остаётся валидной картинкой.
    let natural = svg_size(markup);
    let image = DecodedImage::Vector(Arc::new(VectorImage {
        markup: markup.into(),
        natural,
    }));
    let metadata = ImageMetadata {
        natural,
        applied_orientation: None,
        vector: true,
    };
    Ok((image, metadata))
}

fn codec_error(error: image::ImageError) -> ImageError {
    match error {
        image::ImageError::Unsupported(_) => ImageError::InvalidData,
        image::ImageError::Limits(_) => ImageError::LimitExceeded,
        image::ImageError::Decoding(_)
        | image::ImageError::Parameter(_)
        | image::ImageError::Encoding(_)
        | image::ImageError::IoError(_) => ImageError::Decode,
    }
}
