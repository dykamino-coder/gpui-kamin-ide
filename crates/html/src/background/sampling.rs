//! Preserve pixel boundaries for an unscaled CSS background raster.

use gpui::{Bounds, Corners, ImageSampling, Pixels, RenderImage, Window};
use std::sync::Arc;

pub(super) fn snapped_clip(bounds: Bounds<Pixels>, window: &Window) -> Bounds<Pixels> {
    if window.current_transformation() != gpui::TransformationMatrix::unit() {
        return bounds;
    }
    // CSS Backgrounds 3 §3.7 clips to the painted box. Resolve the logical
    // positioning area separately, then snap this clip like the box edges:
    // a fractional clip can otherwise discard its last painted device column.
    let scale = window.scale_factor();
    let edge = |v: Pixels| gpui::px((f32::from(v) * scale).round() / scale);
    Bounds::from_corners(
        gpui::point(edge(bounds.left()), edge(bounds.top())),
        gpui::point(edge(bounds.right()), edge(bounds.bottom())),
    )
}

pub(super) fn paint_tile(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    image: Arc<RenderImage>,
    source_kind: &super::Source,
) {
    let source = image.size(0);
    let mut sampling = sampling_mode(
        (source.width.0 as f32, source.height.0 as f32),
        (f32::from(bounds.size.width), f32::from(bounds.size.height)),
        window.current_transformation().rotation_scale,
        matches!(source_kind, super::Source::Raster(_)),
    );
    // CSS Backgrounds 3 §3.8 resolves the positioning area before painting.
    // Snap both final edges together (Blink background_image_geometry.cc:756),
    // rather than flooring the origin and independently expanding the size.
    if window.current_transformation().rotation_scale == [[1.0, 0.0], [0.0, 1.0]] {
        sampling = match sampling {
            ImageSampling::Nearest => ImageSampling::NearestSnapped,
            _ => ImageSampling::LinearSnapped,
        };
    }
    let _ = window.paint_image_with_sampling(bounds, corners, image, 0, false, sampling);
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
