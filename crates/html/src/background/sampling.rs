//! Preserve pixel boundaries for an unscaled CSS background raster.

use gpui::{Bounds, Corners, ImageSampling, Pixels, RenderImage, Window};
use std::sync::Arc;

pub(super) fn paint_tile(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    image: Arc<RenderImage>,
    source_kind: &super::Source,
) {
    let source = image.size(0);
    let sampling = sampling_mode(
        (source.width.0 as f32, source.height.0 as f32),
        (f32::from(bounds.size.width), f32::from(bounds.size.height)),
        window.current_transformation().rotation_scale,
        matches!(source_kind, super::Source::Raster(_)),
    );
    let bounds = if sampling == ImageSampling::Nearest
        && window.current_transformation() == gpui::TransformationMatrix::unit()
    {
        // CSS 2.1 section 14.2 defines the positioning area before painting.
        // Blink background_image_geometry.cc:114-122 snaps the destination
        // origin while retaining the tile size and source-image mapping.
        // GPUI floors sprite origins, biasing fractional background offsets
        // toward the preceding device pixel even beside snapped CSS borders.
        let scale = window.scale_factor();
        let edge = |value: Pixels| {
            let physical = (f32::from(value) * scale).round();
            let mut logical = physical / scale;
            // Preserve the chosen integer through GPUI's later multiply/floor.
            if logical * scale < physical {
                logical = logical.next_up();
            }
            gpui::px(logical)
        };
        // The far edge snaps the same way, as the edges of a box laid out at
        // the tile's place: keeping the unsnapped size after the snapped
        // origin painted a 15px tile at 1.25 over 19 device rows where the
        // same image as a box covers 18 (background-position-applies-to-*).
        // GPUI ceils the sprite size: the device span is kept from rounding
        // up past the chosen integer.
        let span = |origin: Pixels, size: Pixels| {
            let from = (f32::from(origin) * scale).round();
            let to = ((f32::from(origin) + f32::from(size)) * scale).round();
            let device = (to - from).max(0.0);
            let mut logical = device / scale;
            if logical * scale > device {
                logical = logical.next_down();
            }
            gpui::px(logical)
        };
        Bounds {
            origin: bounds.origin.map(edge),
            size: gpui::size(
                span(bounds.origin.x, bounds.size.width),
                span(bounds.origin.y, bounds.size.height),
            ),
        }
    } else {
        bounds
    };
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
