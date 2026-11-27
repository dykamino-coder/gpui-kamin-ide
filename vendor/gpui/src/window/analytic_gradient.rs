//! Analytic fills retain the immutable resource owned by their paint command.

use super::*;

impl Window {
    /// Paint resolved sRGB geometry directly in the target, without a bitmap tile.
    /// The immutable payload survives retained replay and device recreation.
    pub fn paint_analytic_gradient(
        &mut self,
        bounds: Bounds<Pixels>,
        corners: Corners<Pixels>,
        gradient: Arc<crate::AnalyticGradient>,
    ) {
        self.invalidator.debug_assert_paint();
        let scale = self.scale_factor();
        let mut background = Background::default();
        background.tag = crate::color::BackgroundTag::AnalyticGradient;
        background.solid.a = self.element_opacity();
        self.insert_scene_primitive(crate::Primitive::Gradient(crate::GradientQuad {
            quad: Quad {
                order: 0,
                bounds: bounds.scale(scale),
                content_mask: self.content_mask().scale(scale),
                background,
                border_color: Hsla::default(),
                border_widths: Edges::default(),
                corner_radii: corners.scale(scale),
                border_style: BorderStyle::Solid,
                ellipse: Default::default(),
                transformation: self.current_transformation(),
            },
            gradient,
        }));
    }
}
