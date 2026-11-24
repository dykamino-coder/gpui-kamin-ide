//! Image bytes to a neutral value: an SVG picture is recognised by its content (a
//! `data:` URL has no extension), a raster by the codecs of the `image` crate (the same
//! formats as GPUI). At load a raster is read only up to its HEADER: its size, pixel
//! format and EXIF orientation give the natural size after rotation (css-images-3
//! §5.4) and the admission of the declared size (`admission`); the pixels are decoded
//! on demand by [`super::encoded::EncodedRaster`] through [`decode_pixels`]: pixels ->
//! colour profile -> orientation -> channel order.

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

/// Parse image bytes; `admit` sees the declared raster before any pixels.
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
    // A 90 degree turn (tags 5..8) swaps the sides.
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

/// The header of a raster: its codec, declared raster and effective EXIF tag.
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

/// The pixels of a raster admitted at load: `retention` is its physical quota.
pub(super) fn decode_pixels(
    bytes: &[u8],
    options: &DecodeOptions,
    retention: Retention,
) -> Result<CpuRaster, ImageError> {
    let Pixels {
        width,
        height,
        format,
        colour_space,
        pixels,
    } = decode_vec(bytes, options)?;
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

/// Decoded straight-alpha pixels in the buffer the codec filled: four bytes a pixel,
/// rows without padding.
pub(super) struct Pixels {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: PixelFormat,
    pub(super) colour_space: ColourSpace,
    pub(super) pixels: Vec<u8>,
}

/// The pixels as an owned buffer: a one-off consumer (the texture upload) takes them
/// without the copy into a shared raster.
pub(super) fn decode_vec(bytes: &[u8], options: &DecodeOptions) -> Result<Pixels, ImageError> {
    let (mut decoder, declared, tag) = header(bytes, options)?;
    // Как `ImageReader::decode`: стандартный бюджет выделений кодека, а стороны не больше
    // заявленных и допущенных.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(declared.width);
    limits.max_image_height = Some(declared.height);
    limits.reserve(decoder.total_bytes()).map_err(codec_error)?;
    decoder.set_limits(limits).map_err(codec_error)?;
    let image = image::DynamicImage::from_decoder(decoder).map_err(codec_error)?;
    // RGBA8 output moves into the raster without a copy; other formats convert once.
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
    Ok(Pixels {
        width,
        height,
        format,
        colour_space,
        pixels,
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
