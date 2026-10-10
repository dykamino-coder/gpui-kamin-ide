//! Image content fit and position are independent of its layout dimensions.
use crate::{AnyElement, Bounds, DefiniteLength, ObjectFit, Pixels, Point};

/// The style of an image element.
pub struct ImageStyle {
    pub(super) grayscale: bool,
    pub(super) preserve_natural_pixels: bool,
    pub(super) object_fit: ObjectFit,
    pub(super) object_position: Option<Point<DefiniteLength>>,
    pub(super) loading: Option<Box<dyn Fn() -> AnyElement>>,
    pub(super) fallback: Option<Box<dyn Fn() -> AnyElement>>,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            grayscale: false,
            preserve_natural_pixels: false,
            object_fit: ObjectFit::Contain,
            object_position: None,
            loading: None,
            fallback: None,
        }
    }
}

/// Style an image element.
pub trait StyledImage: Sized {
    /// Get a mutable [ImageStyle] from the element.
    fn image_style(&mut self) -> &mut ImageStyle;

    /// Set the image to be displayed in grayscale.
    fn grayscale(mut self, grayscale: bool) -> Self {
        self.image_style().grayscale = grayscale;
        self
    }

    /// Preserve raster pixel boundaries at natural size without a linear transform.
    /// Resized or transformed content keeps interpolation. Disabled by default.
    fn preserve_natural_pixels(mut self, preserve: bool) -> Self {
        self.image_style().preserve_natural_pixels = preserve;
        self
    }

    /// Set the object fit for the image.
    fn object_fit(mut self, object_fit: ObjectFit) -> Self {
        self.image_style().object_fit = object_fit;
        self
    }

    /// Position the fitted content; percentages resolve against leftover space.
    fn object_position(mut self, position: Point<DefiniteLength>) -> Self {
        self.image_style().object_position = Some(position);
        self
    }

    /// Display a replacement if the image cannot load.
    fn with_fallback(mut self, fallback: impl Fn() -> AnyElement + 'static) -> Self {
        self.image_style().fallback = Some(Box::new(fallback));
        self
    }

    /// Display a replacement while the image is loading.
    fn with_loading(mut self, loading: impl Fn() -> AnyElement + 'static) -> Self {
        self.image_style().loading = Some(Box::new(loading));
        self
    }
}

pub(super) fn position(
    mut fitted: Bounds<Pixels>,
    container: Bounds<Pixels>,
    position: Option<Point<DefiniteLength>>,
    rem: Pixels,
) -> Bounds<Pixels> {
    if let Some(position) = position {
        let free = container.size - fitted.size;
        fitted.origin = container.origin
            + crate::point(
                position.x.to_pixels(free.width.into(), rem),
                position.y.to_pixels(free.height.into(), rem),
            );
    }
    fitted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DevicePixels, point, px, size};

    #[test]
    fn ratio_derived_box_positions_content_using_its_natural_ratio() {
        let container = Bounds::new(point(px(60.0), px(20.0)), size(px(300.0), px(100.0)));
        let fitted = ObjectFit::Contain.get_bounds(container, size(DevicePixels(20), DevicePixels(50)));
        assert_eq!(fitted.size, size(px(40.0), px(100.0)));
        let placed = position(fitted, container, Some(point(px(0.0).into(), px(0.0).into())), px(16.0));
        assert_eq!(placed.origin, container.origin);
        assert_eq!(position(fitted, container, None, px(16.0)), fitted);
    }

    #[test]
    fn oversized_content_resolves_percentages_against_negative_leftover_space() {
        let container = Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(100.0)));
        let fitted = ObjectFit::Cover.get_bounds(container, size(DevicePixels(200), DevicePixels(100)));
        let placed = position(fitted, container, Some(point(DefiniteLength::Fraction(1.0), DefiniteLength::Fraction(0.0))), px(16.0));
        assert_eq!(placed.origin, point(px(-90.0), px(20.0)));
        assert_eq!(placed.size, size(px(200.0), px(100.0)));
    }

    #[test]
    fn edge_offsets_keep_percentage_and_pixel_terms_until_paint() {
        let container = Bounds::new(point(px(10.0), px(20.0)), size(px(300.0), px(100.0)));
        let fitted = Bounds::new(container.origin, size(px(40.0), px(100.0)));
        let placed = position(fitted, container, Some(point(DefiniteLength::Calc(-2.0, 1.0), px(3.0).into())), px(16.0));
        assert_eq!(placed.origin, point(px(268.0), px(23.0)));
    }
}
