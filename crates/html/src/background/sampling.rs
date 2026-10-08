//! Preserve tile pixel boundaries and paint supported gradients analytically.

use gpui::{Bounds, Corners, ImageSampling, Pixels, RenderImage, Window};
use std::sync::Arc;

pub(super) fn paint_tile(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    image: Arc<RenderImage>,
    source_kind: &super::Source,
) {
    if paint_gradient(window, bounds, corners, source_kind) {
        return;
    }
    if super::oriented_vector::paint(window, bounds, corners, source_kind) {
        return;
    }
    let source = image.size(0);
    let sampling = sampling_mode(
        (source.width.0 as f32, source.height.0 as f32),
        (f32::from(bounds.size.width), f32::from(bounds.size.height)),
        window.current_transformation().rotation_scale,
        matches!(source_kind, super::Source::Raster(_)),
    );
    if sampling == ImageSampling::Nearest
        && window.current_transformation() == gpui::TransformationMatrix::unit()
    {
        let _ = window.paint_natural_image(bounds, corners, image, 0, false);
        return;
    }
    // An interpolated tile snaps both final edges together as well (CSS
    // Backgrounds 3 section 3.8; Blink background_image_geometry.cc:756),
    // instead of flooring the origin and independently expanding the size.
    // Ported from fix-near b6801d1; the Nearest path above stays as is.
    let sampling = if sampling == ImageSampling::Linear
        && window.current_transformation().rotation_scale == [[1.0, 0.0], [0.0, 1.0]]
    {
        ImageSampling::LinearSnapped
    } else {
        sampling
    };
    let image = if sampling == ImageSampling::LinearSnapped
        && window.current_transformation() == gpui::TransformationMatrix::unit()
        && matches!(
            source_kind,
            super::Source::Vector { .. } | super::Source::Raster(_)
        ) {
        // CSS Images 3 §image-rendering permits bilinear smooth scaling.
        // Filtering straight RGB darkens an opaque color beside transparent
        // texels. Resolve premultiplied colors first, then upload straight
        // BGRA at the destination resolution expected by GPUI's sprite path.
        let sf = window.scale_factor();
        let w = ((f32::from(bounds.right()) * sf).round() - (f32::from(bounds.left()) * sf).round())
            .max(0.0) as u32;
        let h = ((f32::from(bounds.bottom()) * sf).round() - (f32::from(bounds.top()) * sf).round())
            .max(0.0) as u32;
        super::alpha_sampling::resample(&image, w, h).unwrap_or(image)
    } else {
        image
    };
    let _ = window.paint_image_with_sampling(bounds, corners, image, 0, false, sampling);
}

fn paint_gradient(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    source: &super::Source,
) -> bool {
    let super::Source::Gradient { raw } = source else {
        return false;
    };
    if !raw.starts_with("linear-gradient(") {
        return false;
    }
    let Some(gradient) = crate::computed::parse_gradient(raw) else {
        return false;
    };
    if gradient.stops.len() > 4
        || gradient.stops_raw.iter().any(|(_, _, px)| px.is_some())
        || !matches!(
            gradient.space,
            crate::computed::GradSpace::Srgb | crate::computed::GradSpace::Oklab
        )
        || gradient
            .stops
            .iter()
            .any(|s| crate::color_space::out_of_gamut(s.0))
    {
        return false;
    }
    // CSS Images 3 §3.1 and §3.4.2 define a continuous image over its
    // concrete size. Use the box-gradient shader for supported tiles too:
    // quantizing a CSS-resolution bitmap and then filtering it at device
    // resolution changes the colors relative to an identical box fill.
    // Box fills use Taffy's snapped device edges. Keep that destination
    // convention here as well, including before an ancestor transform.
    let scale = window.scale_factor();
    let edge = |value: Pixels| gpui::px((f32::from(value) * scale).round() / scale);
    let bounds = Bounds::from_corners(
        gpui::point(edge(bounds.left()), edge(bounds.top())),
        gpui::point(edge(bounds.right()), edge(bounds.bottom())),
    );
    let mut quad = gpui::fill(bounds, crate::apply::fill(&gradient));
    quad.corner_radii = corners;
    window.paint_quad(quad);
    true
}

pub(super) fn snapped_clip(bounds: Bounds<Pixels>, window: &Window) -> Bounds<Pixels> {
    if window.current_transformation() != gpui::TransformationMatrix::unit() {
        return bounds;
    }
    // CSS Backgrounds 3 section 3.7 clips to the painted box. Snap the clip
    // like the box edges: a fractional clip can otherwise discard its last
    // painted device column (fix-near b6801d1).
    let scale = window.scale_factor();
    let edge = |v: Pixels| gpui::px((f32::from(v) * scale).round() / scale);
    Bounds::from_corners(
        gpui::point(edge(bounds.left()), edge(bounds.top())),
        gpui::point(edge(bounds.right()), edge(bounds.bottom())),
    )
}

fn sampling_mode(
    source: (f32, f32),
    target: (f32, f32),
    matrix: [[f32; 2]; 2],
    raster: bool,
) -> ImageSampling {
    // CSS intrinsic dimensions establish an unscaled raster even when the
    // window's device scale is fractional. Transforms and CSS resizing still
    // need interpolation; translation alone does not resize the image.
    if raster && source == target && matrix == [[1.0, 0.0], [0.0, 1.0]] {
        ImageSampling::Nearest
    } else {
        ImageSampling::Linear
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizing_either_axis_or_transforming_retains_interpolation() {
        let identity = [[1.0, 0.0], [0.0, 1.0]];
        assert_eq!(
            sampling_mode((320.0, 320.0), (320.0, 320.0), identity, true),
            ImageSampling::Nearest
        );
        for target in [
            (400.0, 320.0),
            (320.0, 400.0),
            (160.0, 160.0),
            (320.25, 320.0),
        ] {
            assert_eq!(
                sampling_mode((320.0, 320.0), target, identity, true),
                ImageSampling::Linear
            );
        }
        for matrix in [
            [[2.0, 0.0], [0.0, 2.0]],
            [[0.0, -1.0], [1.0, 0.0]],
            [[1.0, 0.5], [0.0, 1.0]],
        ] {
            assert_eq!(
                sampling_mode((320.0, 320.0), (320.0, 320.0), matrix, true),
                ImageSampling::Linear
            );
        }
        assert_eq!(
            sampling_mode((320.0, 320.0), (320.0, 320.0), identity, false),
            ImageSampling::Linear
        );
    }
}
