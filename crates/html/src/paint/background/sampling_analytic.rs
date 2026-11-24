//! Arbitrary sRGB stops execute in the target without an intermediate bitmap tile.
//! Immutable records retain CSS geometry independently of raster size or atlas position.

use crate::resources::css_images::gradient_geometry::Geometry;
use crate::style::computed::{GradSpace, Gradient};
use crate::values::ValueCx;
use gpui::{AnalyticGradient, Bounds, Corners, Pixels, Window};

pub(super) fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    g: &Gradient,
    repeating: bool,
    css: (f32, f32),
    vcx: ValueCx<'_>,
) -> bool {
    if g.space != GradSpace::Srgb
        || g.polar.is_some()
        || g.stops
            .iter()
            .any(|s| crate::style::values::color_space::out_of_gamut(s.0))
    {
        return false;
    }
    let Some(geometry) = Geometry::new(g, css, g.angle_deg, vcx) else {
        return false;
    };
    let (geometry, radial) = match geometry {
        Geometry::Linear {
            direction: (x, y),
            length,
        } => ([x, y, length, 0.0], false),
        Geometry::Radial {
            center: (x, y),
            radii: (rx, ry),
        } => ([x, y, rx, ry], true),
    };
    let stops = g.stops.iter().map(|(c, t)| [c.r, c.g, c.b, c.a, *t]);
    let Some(gradient) = AnalyticGradient::new(geometry, [css.0, css.1], radial, repeating, stops)
    else {
        return false;
    };
    window.paint_analytic_gradient(bounds, corners, gradient);
    true
}
