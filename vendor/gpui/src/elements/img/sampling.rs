//! Optional natural-size sampling follows fitted content, not its outer box.
use crate::{Bounds, DevicePixels, ImageSampling, Pixels, RenderImage, Size, Style, Window};
use std::sync::Arc;

pub(super) fn unrounded_bounds(
    window: &mut Window,
    id: crate::LayoutId,
    bounds: Bounds<Pixels>,
) -> Option<Bounds<Pixels>> {
    // A separately placed image (`prepaint_at`) can have a different
    // coordinate frame. Only recover layout phase for its actual layout box.
    if window.layout_bounds(id) != bounds {
        return None;
    }
    let exact = Bounds {
        origin: window.layout_origin_unrounded(id),
        size: window.layout_size_unrounded(id),
    };
    Some(exact)
}

pub(super) fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    exact: Option<Bounds<Pixels>>,
    style: &Style,
    image: Arc<RenderImage>,
    frame: usize,
    image_style: &super::ImageStyle,
) -> anyhow::Result<()> {
    let exact =
        exact.filter(|_| window.current_transformation() == crate::TransformationMatrix::unit());
    let corners = style
        .corner_radii
        .to_pixels(window.rem_size())
        .clamp_radii_for_quad_size(bounds.size);
    let sampling = mode(
        image_style.preserve_natural_pixels,
        image.size(frame),
        exact.unwrap_or(bounds).size,
        window.current_transformation().rotation_scale,
    );
    if sampling == ImageSampling::Nearest {
        return window.paint_natural_image(
            exact.unwrap_or(bounds),
            corners,
            image,
            frame,
            image_style.grayscale,
        );
    }
    window.paint_image_with_sampling(
        bounds,
        corners,
        image,
        frame,
        image_style.grayscale,
        sampling,
    )
}

pub(super) fn mode(
    preserve: bool,
    source: Size<DevicePixels>,
    fitted: Size<Pixels>,
    matrix: [[f32; 2]; 2],
) -> ImageSampling {
    if preserve
        && source.width.0 as f32 == f32::from(fitted.width)
        && source.height.0 as f32 == f32::from(fitted.height)
        && matrix == [[1.0, 0.0], [0.0, 1.0]]
    {
        ImageSampling::Nearest
    } else {
        ImageSampling::Linear
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, ObjectFit, point, px, size};

    #[test]
    fn fitted_object_controls_sampling_independently_of_letterbox_dimensions() {
        let source = size(DevicePixels(100), DevicePixels(100));
        let container = Bounds::new(point(px(10.0), px(20.0)), size(px(200.0), px(100.0)));
        let identity = [[1.0, 0.0], [0.0, 1.0]];
        for (fit, expected) in [
            (ObjectFit::Contain, ImageSampling::Nearest),
            (ObjectFit::None, ImageSampling::Nearest),
            (ObjectFit::Fill, ImageSampling::Linear),
            (ObjectFit::Cover, ImageSampling::Linear),
        ] {
            let fitted = fit.get_bounds(container, source);
            assert_eq!(mode(true, source, fitted.size, identity), expected);
            assert_eq!(
                mode(false, source, fitted.size, identity),
                ImageSampling::Linear
            );
        }
    }

    #[test]
    fn fractional_resize_and_nonidentity_linear_transform_keep_interpolation() {
        let source = size(DevicePixels(100), DevicePixels(100));
        let natural = size(px(100.0), px(100.0));
        assert_eq!(
            mode(
                true,
                source,
                size(px(100.0), px(100.25)),
                [[1.0, 0.0], [0.0, 1.0]]
            ),
            ImageSampling::Linear
        );
        for matrix in [[[2.0, 0.0], [0.0, 2.0]], [[0.0, -1.0], [1.0, 0.0]]] {
            assert_eq!(mode(true, source, natural, matrix), ImageSampling::Linear);
        }
    }
}
