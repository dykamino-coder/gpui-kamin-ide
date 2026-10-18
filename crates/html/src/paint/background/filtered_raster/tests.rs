//! Filter image sizing, coverage and allocation-density regressions.

use super::*;
#[test]
fn concrete_size_keeps_shadow_lengths_in_css_pixels() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let original = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let input = Source::Raster(original);
    let filtered = Source::Filtered {
        image: Box::new(input),
        list: "drop-shadow(5px 0 0 green)".into(),
    };
    assert_eq!(filtered.intrinsic().w, Some(2.0));
    let result = filtered.sampled_raster((10.0, 10.0), true, vcx).unwrap();
    assert!(
        result
            .as_bytes(0)
            .unwrap()
            .chunks_exact(4)
            .all(|p| p == [0, 128, 0, 255])
    );
}
#[test]
fn smooth_scaling_preserves_color_beside_transparency() {
    let input = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let result = resize(&input, 4, 1, false).unwrap();
    let pixels = result.as_bytes(0).unwrap();
    assert_eq!(&pixels[4..8], &[0, 128, 0, 191]);
    assert_eq!(&pixels[8..12], &[0, 128, 0, 64]);
}

#[test]
fn allocation_limit_preserves_css_shadow_offset() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let original = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let input = Source::Raster(original);
    let result = raster(
        &input,
        "drop-shadow(1500px 0 0 green)",
        (3000.0, 10.0),
        false,
        vcx,
    )
    .unwrap();
    assert_eq!(result.size(0).width.0, 2048);
    assert!(
        result
            .as_bytes(0)
            .unwrap()
            .chunks_exact(4)
            .all(|p| p == [0, 128, 0, 255])
    );
}

#[test]
fn radial_input_filters_rendered_samples_without_natural_dimensions() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let source =
        super::super::source("filter(radial-gradient(red, orange), invert(1))", vcx).unwrap();
    assert!(source.intrinsic().w.is_none());
    let filtered = source.raster((7.0, 5.0), vcx).unwrap();
    let original =
        super::super::gradient_raster::tile("radial-gradient(red, orange)", 7, 5, (7.0, 5.0), vcx)
            .unwrap();
    for (a, b) in filtered
        .as_bytes(0)
        .unwrap()
        .chunks_exact(4)
        .zip(original.as_bytes(0).unwrap().chunks_exact(4))
    {
        assert_eq!(a[3], b[3]);
        for c in 0..3 {
            assert_eq!(a[c], 255 - b[c]);
        }
    }
}
