//! Rasterize axis-preserving SVG backgrounds in their final orientation.

use gpui::{Bounds, Corners, Pixels, TransformationMatrix, Window, point, px, size};

pub(super) fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    source: &super::Source,
) -> bool {
    let super::Source::Vector { markup, .. } = source else {
        return false;
    };
    if corners != Corners::default() || markup.to_ascii_lowercase().contains("filter") {
        return false;
    }
    let transform = window.current_transformation();
    // Cardinal rotations retain small sin/cos residues in f32. Accept only
    // unit orthogonal matrices within their arithmetic precision, leaving
    // scaling, skew and arbitrary rotations on the existing image path.
    let canonical = |v: f32| {
        let rounded = v.round();
        ((v - rounded).abs() <= 8.0 * f32::EPSILON && rounded.abs() <= 1.0).then_some(rounded)
    };
    let [[a, b], [c, d]] = transform.rotation_scale;
    let (Some(a), Some(b), Some(c), Some(d)) =
        (canonical(a), canonical(b), canonical(c), canonical(d))
    else {
        return false;
    };
    if (a * d - b * c).abs() != 1.0
        || a.abs() + b.abs() != 1.0
        || c.abs() + d.abs() != 1.0
        || [[a, b], [c, d]] == [[1.0, 0.0], [0.0, 1.0]]
    {
        return false;
    }
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if w <= 0.0 || h <= 0.0 || w > 2048.0 || h > 2048.0 {
        return false;
    }
    let (ow, oh) = (a.abs() * w + b.abs() * h, c.abs() * w + d.abs() * h);
    let (left, top) = (
        (a * w).min(0.0) + (b * h).min(0.0),
        (c * w).min(0.0) + (d * h).min(0.0),
    );
    let viewport = super::with_viewport(markup, (w, h));
    let Ok(document) = roxmltree::Document::parse(&viewport) else {
        return false;
    };
    let inner = &viewport[document.root_element().range()];
    // CSS Transforms 1 §3 transforms the background itself. SVG 2 §8.1
    // maps its vectors into the final viewport before coverage is sampled.
    // Rotating an already rasterized SVG instead retains coverage and
    // transparent RGB from the former orientation, unlike the same vectors
    // drawn directly in the final orientation. Keep integer raster density;
    // SVG filter chains require a separate transformed intermediate surface.
    let oriented = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{ow}\" height=\"{oh}\" viewBox=\"0 0 {ow} {oh}\"><g transform=\"matrix({a} {c} {b} {d} {} {})\">{inner}</g></svg>",
        -left, -top,
    );
    let Some(image) = crate::svg::raster::rasterize(&oriented, ow, oh) else {
        return false;
    };
    let scale = window.scale_factor();
    let origin = transform.apply(point(bounds.origin.x * scale, bounds.origin.y * scale));
    let final_bounds = Bounds {
        origin: point(origin.x / scale + px(left), origin.y / scale + px(top)),
        size: size(px(ow), px(oh)),
    };
    window.with_absolute_transformation(TransformationMatrix::unit(), |window| {
        super::sampling::paint_tile(window, final_bounds, Corners::default(), image, source);
    });
    true
}
