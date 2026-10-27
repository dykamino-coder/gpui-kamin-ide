//! Размеры фильтра, покрытие, плотность и числовые ключи кэша.

use super::*;

#[test]
fn numeric_filters_hit_with_nan_and_signed_zero_without_hiding_changes() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let input =
        Source::Raster(crate::resources::images::pixels::bgra(1, 1, vec![0, 128, 0, 255]).unwrap());
    let mut list = crate::style::computed::ImageFilter::parse("invert(0)", vcx);
    for values in [[0.0, -0.0], [f32::NAN, f32::from_bits(0xff80_0123)]] {
        let mut images = Vec::new();
        for value in values {
            list.filter.as_mut().unwrap().invert = value;
            images.push(raster(&input, &list, (3.0, 2.0), false, vcx).unwrap());
        }
        assert!(Arc::ptr_eq(&images[0], &images[1]));
        assert_eq!(images[0].bytes, images[1].bytes);
    }
    list.filter.as_mut().unwrap().invert = 0.0;
    let original = raster(&input, &list, (3.0, 2.0), false, vcx).unwrap();
    list.filter.as_mut().unwrap().invert = 1.0;
    let inverted = raster(&input, &list, (3.0, 2.0), false, vcx).unwrap();
    assert_ne!(original.bytes, inverted.bytes);
}

#[test]
fn fractional_tile_scale_is_part_of_shadow_key() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let image =
        crate::resources::images::pixels::bgra(3, 1, vec![0, 128, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0])
            .unwrap();
    let input = Source::Raster(image.clone());
    let list = crate::style::computed::ImageFilter::parse("drop-shadow(1.5px 0 0 green)", vcx);
    let mut results = Vec::new();
    for width in [2.6, 3.4] {
        let cached = raster(&input, &list, (width, 1.0), false, vcx).unwrap();
        let direct = super::super::filter_image::apply(&image, &list, (3.0 / width, 1.0)).unwrap();
        assert_eq!(cached.bytes, direct.bytes);
        results.push(cached);
    }
    assert_ne!(results[0].bytes, results[1].bytes);
}
#[test]
fn concrete_size_keeps_shadow_lengths_in_css_pixels() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let original =
        crate::resources::images::pixels::bgra(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let input = Source::Raster(original);
    let filtered = Source::Filtered {
        image: Box::new(input),
        list: Box::new(crate::style::computed::ImageFilter::parse(
            "drop-shadow(5px 0 0 green)",
            vcx,
        )),
    };
    assert_eq!(filtered.intrinsic().w, Some(2.0));
    let result = filtered.sampled_raster((10.0, 10.0), true, vcx).unwrap();
    assert!(
        result
            .bytes
            .as_ref()
            .chunks_exact(4)
            .all(|p| p == [0, 128, 0, 255])
    );
}
#[test]
fn smooth_scaling_preserves_color_beside_transparency() {
    let input =
        crate::resources::images::pixels::bgra(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let result = resize(&input, 4, 1, false).unwrap();
    let pixels = result.bytes.as_ref();
    assert_eq!(&pixels[4..8], &[0, 128, 0, 191]);
    assert_eq!(&pixels[8..12], &[0, 128, 0, 64]);
}

#[test]
fn allocation_limit_preserves_css_shadow_offset() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let original =
        crate::resources::images::pixels::bgra(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let input = Source::Raster(original);
    let result = raster(
        &input,
        &crate::style::computed::ImageFilter::parse("drop-shadow(1500px 0 0 green)", vcx),
        (3000.0, 10.0),
        false,
        vcx,
    )
    .unwrap();
    assert_eq!(result.width, 2048);
    assert!(
        result
            .bytes
            .as_ref()
            .chunks_exact(4)
            .all(|p| p == [0, 128, 0, 255])
    );
}

#[test]
fn radial_input_filters_rendered_samples_without_natural_dimensions() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let source = super::super::computed_source(
        &crate::style::computed::ImageSource::parse(
            "filter(radial-gradient(red, orange), invert(1))",
            vcx,
        )
        .unwrap(),
        true,
        vcx,
    )
    .unwrap();
    assert!(source.intrinsic().w.is_none());
    let filtered = source.raster((7.0, 5.0), vcx).unwrap();
    let original = super::super::gradient_raster::tile(
        &crate::style::computed::ImageSource::parse("radial-gradient(red, orange)", vcx).unwrap(),
        7,
        5,
        (7.0, 5.0),
        vcx,
    )
    .unwrap();
    for (a, b) in filtered
        .bytes
        .as_ref()
        .chunks_exact(4)
        .zip(original.bytes.as_ref().chunks_exact(4))
    {
        assert_eq!(a[3], b[3]);
        for c in 0..3 {
            assert_eq!(a[c], 255 - b[c]);
        }
    }
}
