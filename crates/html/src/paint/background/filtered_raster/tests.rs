//! Filter image sizing, coverage and allocation-density regressions.

use super::*;
#[test]
fn concrete_size_keeps_shadow_lengths_in_css_pixels() {
    let original = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let input = Source::Raster(original);
    let filtered = Source::Filtered {
        image: Box::new(input),
        list: "drop-shadow(5px 0 0 green)".into(),
    };
    assert_eq!(filtered.intrinsic().w, Some(2.0));
    let result = filtered.sampled_raster((10.0, 10.0), true).unwrap();
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
    let original = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
    let input = Source::Raster(original);
    let result = raster(
        &input,
        "drop-shadow(1500px 0 0 green)",
        (3000.0, 10.0),
        false,
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
