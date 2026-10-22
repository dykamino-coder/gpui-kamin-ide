//! Декодер на байтовых образцах: `data:`/`file:` через загрузчик, пустые и битые байты,
//! ориентация EXIF 1–8 (PNG и JPEG) до природного размера, профиль ICC, собственный
//! размер и вырожденный `viewBox` SVG, вид `#view`.

use super::fixtures;

use super::decode::decode;
use super::svg_view::apply_view;
use super::types::*;
use super::vector::svg_size;
use crate::resources::loader::{ResourceLoader, ResourceUrl, SyncLoader};
use fixtures::*;

fn raster(bytes: &[u8], options: DecodeOptions) -> (std::sync::Arc<CpuRaster>, ImageMetadata) {
    match decode(bytes, &options).unwrap() {
        (DecodedImage::Raster(r), meta) => (r, meta),
        (DecodedImage::Vector(_), _) => panic!("raster expected"),
    }
}

#[test]
fn data_and_file_urls_decode_through_the_loader() {
    let png = png(3, 2);
    let (url, _) = ResourceUrl::resolve(&data_url("image/png", &png), None);
    let loaded = SyncLoader.load(&url).unwrap();
    let (image, meta) = raster(&loaded.bytes, DecodeOptions::default());
    assert_eq!((image.width, image.height), (3, 2));
    assert_eq!(image.pixel(2, 1), Some(pixel_at(2, 1)));
    assert_eq!((meta.natural.w, meta.natural.h), (Some(3.0), Some(2.0)));
    assert_eq!(image.stride, 12);
    assert_eq!(
        (image.format, image.alpha),
        (PixelFormat::Rgba8, AlphaMode::Straight)
    );

    let path = std::env::temp_dir().join(format!("kamin-images-{}-a b.png", std::process::id()));
    std::fs::write(&path, &png).unwrap();
    let file = url::Url::from_file_path(&path).unwrap().to_string();
    let (url, _) = ResourceUrl::resolve(&file, None);
    let loaded = SyncLoader.load(&url).unwrap();
    assert_eq!(raster(&loaded.bytes, DecodeOptions::default()).0.width, 3);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn empty_corrupt_and_foreign_bytes_are_typed_errors() {
    let options = DecodeOptions::default();
    assert_eq!(decode(b"", &options).unwrap_err(), ImageError::InvalidData);
    assert_eq!(
        decode(b"not an image", &options).unwrap_err(),
        ImageError::InvalidData
    );
    let png = png(4, 4);
    let truncated = &png[..png.len() - 20];
    assert_eq!(decode(truncated, &options).unwrap_err(), ImageError::Decode);
    // Looks like SVG but is not UTF-8.
    assert_eq!(
        decode(b"<svg>\xFF</svg>", &options).unwrap_err(),
        ImageError::InvalidData
    );
}

#[test]
fn exif_orientation_1_to_8_turns_pixels_before_natural_size() {
    // TIFF 6.0: the stored pixel shown at the top-left corner for each tag.
    let corner = [
        (0, 0),
        (0, 0),
        (2, 0),
        (2, 1),
        (0, 1),
        (0, 0),
        (0, 1),
        (2, 1),
        (2, 0),
    ];
    for tag in 1..=8u16 {
        let bytes = with_chunk(&png(3, 2), b"eXIf", &tiff_orientation(tag));
        let (image, meta) = raster(&bytes, DecodeOptions::default());
        let swapped = tag >= 5;
        let expected = if swapped { (2, 3) } else { (3, 2) };
        assert_eq!((image.width, image.height), expected, "tag {tag}");
        assert_eq!(meta.natural.w, Some(expected.0 as f32), "tag {tag}");
        let (x, y) = corner[tag as usize];
        assert_eq!(image.pixel(0, 0), Some(pixel_at(x, y)), "tag {tag}");
        assert_eq!(meta.applied_orientation, (tag > 1).then_some(tag));
        // image-orientation: none keeps the stored pixels and dimensions.
        let options = DecodeOptions {
            orientation: OrientationMode::None,
            ..DecodeOptions::default()
        };
        let (raw, meta) = raster(&bytes, options);
        assert_eq!(
            (raw.width, raw.height, meta.applied_orientation),
            (3, 2, None)
        );
    }
    let jpeg = jpeg_with_orientation(4, 2, 6);
    let (image, meta) = raster(&jpeg, DecodeOptions::default());
    assert_eq!(
        (image.width, image.height, meta.applied_orientation),
        (2, 4, Some(6))
    );
}

#[test]
fn embedded_icc_profile_converts_to_srgb_or_is_preserved() {
    let profile = linear_srgb_profile();
    let bytes = with_chunk(&solid_png([128, 128, 128, 77]), b"iCCP", &iccp(&profile));
    let (image, _) = raster(&bytes, DecodeOptions::default());
    // A linear-light grey of 128/255 is about 188 after the sRGB transfer curve.
    let [r, g, b, a] = image.pixel(0, 0).unwrap();
    for c in [r, g, b] {
        assert!((187..=189).contains(&c), "{c}");
    }
    assert_eq!(a, 77, "the profile never changes alpha");
    assert_eq!(image.colour_space, ColourSpace::Srgb);
    let preserve = DecodeOptions {
        colour: ColourPolicy::Preserve,
        ..DecodeOptions::default()
    };
    let (kept, _) = raster(&bytes, preserve);
    assert_eq!(kept.pixel(0, 0), Some([128, 128, 128, 77]));
    assert_eq!(kept.colour_space, ColourSpace::Embedded(profile.into()));
}

#[test]
fn svg_natural_size_ratio_and_degenerate_view_box() {
    let decoded = |markup: &str| decode(markup.as_bytes(), &DecodeOptions::default());
    let (image, meta) = decoded("<?xml version='1.0'?><svg width='20px' height='10'/>").unwrap();
    assert!(matches!(image, DecodedImage::Vector(_)) && meta.vector);
    assert_eq!(
        (meta.natural.w, meta.natural.h, meta.natural.ratio),
        (Some(20.0), Some(10.0), Some(2.0))
    );
    // A percentage is not a natural dimension; the viewBox still gives a ratio.
    let (_, meta) = decoded("<svg width='50%' viewBox='0 0 30 10'/>").unwrap();
    assert_eq!(
        (meta.natural.w, meta.natural.h, meta.natural.ratio),
        (None, None, Some(3.0))
    );
    let (_, meta) = decoded("<svg/>").unwrap();
    assert_eq!(meta.natural, super::natural::Intrinsic::default());
    assert_eq!(
        decoded("<svg viewBox='0 0 0 10'/>").unwrap_err(),
        ImageError::EmptyViewBox
    );
}

#[test]
fn view_fragment_selects_the_view_box_of_the_same_bytes() {
    let svg = "<svg viewBox='0 0 100 50'><view id='tall' viewBox='0 0 10 40'/></svg>";
    let viewed = String::from_utf8(apply_view(svg.as_bytes().to_vec(), "tall")).unwrap();
    assert_eq!(svg_size(&viewed).ratio, Some(0.25));
    assert_eq!(svg_size(svg).ratio, Some(2.0));
    let (url, fragment) = ResourceUrl::resolve("file:///images/a.svg#tall", None);
    assert_eq!(
        (url.as_str(), fragment.as_deref()),
        ("file:///images/a.svg", Some("tall"))
    );
}
