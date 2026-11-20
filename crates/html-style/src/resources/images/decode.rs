//! Декодер картинки из байт в нейтральное значение: рисунок SVG распознаётся по
//! содержимому (у `data:` расширения нет), растр — кодеками крейта `image` (тот же набор
//! форматов, что у GPUI). При загрузке растр читается только до ЗАГОЛОВКА: стороны,
//! формат точек и EXIF-ориентация дают природный размер после поворота (css-images-3
//! §5.4) и допуск заявленного размера (`admission`); сами точки декодирует по требованию
//! [`super::encoded::EncodedRaster`] — тем же путём [`decode_pixels`]: точки → цветовой
//! профиль → ориентация → порядок каналов.

use super::admission::{Declared, Grant, Retention};
use super::colour::{convert_to_srgb, png_icc_profile};
use super::encoded::EncodedRaster;
use super::orientation::{exif_orientation, orient_pixels};
use super::types::{
    AlphaMode, ColourPolicy, ColourSpace, CpuRaster, DecodeOptions, DecodedImage, ImageError,
    ImageLimit, ImageMetadata, OrientationMode, PixelFormat, RasterImage,
};
use super::vector::{VectorImage, looks_svg, svg_size};
use crate::color::IccProfile;
use image::ImageDecoder;
use std::sync::Arc;

/// Разобрать байты картинки; `admit` получает заявленный растр до любых точек.
pub fn decode(
    bytes: &Arc<[u8]>,
    options: &DecodeOptions,
    admit: impl FnOnce(&Declared) -> Result<Grant, ImageError>,
) -> Result<(DecodedImage, ImageMetadata), ImageError> {
    if bytes.is_empty() {
        return Err(ImageError::InvalidData);
    }
    if looks_svg(bytes) {
        return decode_svg(bytes);
    }
    let (_, declared, tag) = header(bytes, options)?;
    let Grant { retention, .. } = admit(&declared)?;
    // Поворот на 90° (метки 5..8) меняет стороны местами.
    let (width, height) = match tag {
        Some(5..=8) => (declared.height, declared.width),
        _ => (declared.width, declared.height),
    };
    let raster = EncodedRaster::new(bytes.clone(), *options, (width, height), retention);
    let image = DecodedImage::Raster(RasterImage::Encoded(Arc::new(raster)));
    let metadata = ImageMetadata {
        natural: image.natural(),
        applied_orientation: tag,
        vector: false,
    };
    Ok((image, metadata))
}

type Header<'a> = (Box<dyn image::ImageDecoder + 'a>, Declared, Option<u16>);

/// Заголовок растра: кодек, заявленный растр и действующая EXIF-метка.
fn header<'a>(bytes: &'a [u8], options: &DecodeOptions) -> Result<Header<'a>, ImageError> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ImageError::Decode)?;
    let decoder = reader.into_decoder().map_err(codec_error)?;
    let (width, height) = decoder.dimensions();
    let tag = (options.orientation == OrientationMode::FromImage)
        .then(|| exif_orientation(bytes))
        .flatten()
        .filter(|tag| (2..=8).contains(tag));
    let declared = Declared {
        width,
        height,
        native: decoder.total_bytes(),
        converts: decoder.color_type() != image::ColorType::Rgba8,
        orients: tag.is_some(),
    };
    Ok((Box::new(decoder), declared, tag))
}

/// Точки растра, допущенного при загрузке: `retention` — его physical квота.
pub(super) fn decode_pixels(
    bytes: &[u8],
    options: &DecodeOptions,
    retention: Retention,
) -> Result<CpuRaster, ImageError> {
    let (mut decoder, declared, tag) = header(bytes, options)?;
    // Как `ImageReader::decode`: стандартный бюджет выделений кодека, а стороны не больше
    // заявленных и допущенных.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(declared.width);
    limits.max_image_height = Some(declared.height);
    limits.reserve(decoder.total_bytes()).map_err(codec_error)?;
    decoder.set_limits(limits).map_err(codec_error)?;
    let image = image::DynamicImage::from_decoder(decoder).map_err(codec_error)?;
    // Выход RGBA8 переходит в растр без копии; прочие форматы переводятся один раз.
    let mut rgba = match image {
        image::DynamicImage::ImageRgba8(rgba) => rgba,
        other => other.into_rgba8(),
    };
    let mut colour_space = ColourSpace::Srgb;
    if let Some(profile) = png_icc_profile(bytes) {
        match (options.colour, IccProfile::parse(&profile)) {
            (ColourPolicy::ConvertToSrgb, Some(parsed)) => convert_to_srgb(&mut rgba, &parsed),
            // Непонятый или сохранённый профиль остаётся описанием точек, а не молча sRGB.
            _ => colour_space = ColourSpace::Embedded(profile.into()),
        }
    }
    let mut pixels = rgba.into_raw();
    let (mut width, mut height) = (declared.width, declared.height);
    if let Some(tag) = tag
        && let Some((w, h, turned)) = orient_pixels(width, height, &pixels, tag)
    {
        (width, height, pixels) = (w, h, turned);
    }
    let format = if options.colour == ColourPolicy::CssBgra {
        super::presentation::css_bgra(&mut pixels, &colour_space);
        colour_space = ColourSpace::Srgb;
        PixelFormat::Bgra8
    } else {
        PixelFormat::Rgba8
    };
    Ok(CpuRaster {
        width,
        height,
        stride: width as usize * 4,
        format,
        alpha: AlphaMode::Straight,
        colour_space,
        bytes: pixels.into(),
        retention,
    })
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
