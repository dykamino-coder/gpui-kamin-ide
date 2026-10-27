//! Декодер картинки из байт в нейтральное значение: рисунок SVG распознаётся по
//! содержимому (у `data:` расширения нет), растр — кодеками крейта `image` (тот же набор
//! форматов, что у GPUI). Порядок для растра: заголовок → допуск заявленного размера
//! (`admission`) → точки → цветовой профиль → EXIF-ориентация, чтобы природный размер
//! считался по развёрнутой картинке (css-images-3 §5.4).

use super::admission::{Declared, Grant};
use super::colour::{convert_to_srgb, png_icc_profile};
use super::orientation::{exif_orientation, orient_pixels};
use super::types::{
    AlphaMode, ColourPolicy, ColourSpace, CpuRaster, DecodeOptions, DecodedImage, ImageError,
    ImageLimit, ImageMetadata, OrientationMode, PixelFormat,
};
use super::vector::{VectorImage, looks_svg, svg_size};
use crate::color::IccProfile;
use image::ImageDecoder;
use std::sync::Arc;

/// Декодировать байты картинки; `admit` получает заявленный растр до выделения точек.
pub(crate) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
    admit: impl FnOnce(&Declared) -> Result<Grant, ImageError>,
) -> Result<(DecodedImage, ImageMetadata), ImageError> {
    if bytes.is_empty() {
        return Err(ImageError::InvalidData);
    }
    if looks_svg(bytes) {
        return decode_svg(bytes);
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ImageError::Decode)?;
    let mut decoder = reader.into_decoder().map_err(codec_error)?;
    let (width, height) = decoder.dimensions();
    let tag = (options.orientation == OrientationMode::FromImage)
        .then(|| exif_orientation(bytes))
        .flatten()
        .filter(|tag| (2..=8).contains(tag));
    let grant = admit(&Declared {
        width,
        height,
        native: decoder.total_bytes(),
        converts: decoder.color_type() != image::ColorType::Rgba8,
        orients: tag.is_some(),
    })?;
    // Как `ImageReader::decode`: стандартный бюджет выделений кодека, а стороны не больше
    // заявленных и допущенных.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.reserve(decoder.total_bytes()).map_err(codec_error)?;
    decoder.set_limits(limits).map_err(codec_error)?;
    let mut rgba = image::DynamicImage::from_decoder(decoder)
        .map_err(codec_error)?
        .into_rgba8();
    let mut colour_space = ColourSpace::Srgb;
    if let Some(profile) = png_icc_profile(bytes) {
        match (options.colour, IccProfile::parse(&profile)) {
            (ColourPolicy::ConvertToSrgb, Some(parsed)) => convert_to_srgb(&mut rgba, &parsed),
            // Непонятый или сохранённый профиль остаётся описанием точек, а не молча sRGB.
            _ => colour_space = ColourSpace::Embedded(profile.into()),
        }
    }
    let mut pixels = rgba.into_raw();
    let (mut width, mut height) = (width, height);
    let mut applied_orientation = None;
    if let Some(tag) = tag
        && let Some((w, h, turned)) = orient_pixels(width, height, &pixels, tag)
    {
        (width, height, pixels) = (w, h, turned);
        applied_orientation = Some(tag);
    }
    let Grant { retention, .. } = grant;
    let raster = CpuRaster {
        width,
        height,
        stride: width as usize * 4,
        format: PixelFormat::Rgba8,
        alpha: AlphaMode::Straight,
        colour_space,
        bytes: pixels.into(),
        retention,
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
        image::ImageError::Limits(_) => ImageError::LimitExceeded(ImageLimit::Codec),
        image::ImageError::Decoding(_)
        | image::ImageError::Parameter(_)
        | image::ImageError::Encoding(_)
        | image::ImageError::IoError(_) => ImageError::Decode,
    }
}
