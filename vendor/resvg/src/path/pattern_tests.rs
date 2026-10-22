//! Subpixel placement and sampling of repeated SVG tiles.
// KaminIDE patch: fractional transforms must retain their continuous position.
#[test]
fn fractional_pattern_translation_preserves_subpixel_edges() {
    for svg in [include_str!("fixtures/fractional-x.svg"), include_str!("fixtures/fractional-y.svg")] {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(24, 24).unwrap();
        crate::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
        let p = pixmap.pixel(4, 4).unwrap();
        assert!(p.red() > 80 && p.blue() > 80, "fractional edge must mix adjacent colours: {p:?}");
    }
}

// KaminIDE patch: alignment is tested after composing both transforms.
#[test]
fn nearest_requires_integer_device_translation() {
    use tiny_skia::{FilterQuality, Transform};
    let quality = |a, b| super::pattern_quality(a, b);
    assert_eq!(quality(Transform::identity(), Transform::from_translate(2.0, -3.0)), FilterQuality::Nearest);
    assert_eq!(quality(Transform::from_scale(-1.0, 1.0), Transform::identity()), FilterQuality::Nearest);
    assert_eq!(quality(Transform::from_translate(0.5, 0.0), Transform::identity()), FilterQuality::Bicubic);
    assert_eq!(quality(Transform::identity(), Transform::from_translate(0.0, -0.5)), FilterQuality::Bicubic);
    assert_eq!(quality(Transform::from_translate(0.5, 0.0), Transform::from_translate(0.5, 0.0)), FilterQuality::Nearest);
}

#[test]
fn device_translation_and_integer_alignment_are_preserved() {
    for (svg, shift) in [
        (include_str!("fixtures/fractional-x.svg"), (0.5, 0.0)),
        (include_str!("fixtures/fractional-y.svg"), (0.0, 0.5)),
    ] {
        let svg = svg.replace("translate(0.5 0)", "translate(0 0)")
            .replace("translate(0 0.5)", "translate(0 0)");
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(24, 24).unwrap();
        crate::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
        assert_eq!(pixmap.pixel(4, 4).unwrap().blue(), 255);
        assert_eq!(pixmap.pixel(3, 3).unwrap().red(), 255);
        pixmap.fill(tiny_skia::Color::TRANSPARENT);
        crate::render(&tree, tiny_skia::Transform::from_translate(shift.0, shift.1), &mut pixmap.as_mut());
        let p = pixmap.pixel(4, 4).unwrap();
        assert!(p.red() > 80 && p.blue() > 80, "device translation lost: {p:?}");
    }
}
