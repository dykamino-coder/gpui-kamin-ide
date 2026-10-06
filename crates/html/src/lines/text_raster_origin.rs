//! Align horizontal HTML text baselines with the device pixel grid at paint time.

use super::*;

impl Paragraph {
    pub(super) fn text_raster_origin(
        &self,
        shaped: &gpui::ShapedLine,
        origin: Point<Pixels>,
        window: &Window,
    ) -> Point<Pixels> {
        if self.selection_vertical.is_some() || crate::interact::in_rotated_frame() {
            return origin;
        }
        // GPUI has no vertical subpixel variants on Windows/Linux; flooring the
        // baseline biases fractional half-leading upward. CSS 2.1 section 10.8.1
        // defines the baseline before rasterization, so preserve it in layout
        // and choose its nearest device pixel only for painting.
        let base = (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        let scale = window.scale_factor();
        let y = f32::from(origin.y + base) * scale;
        point(origin.x, origin.y + px((y.round() - y) / scale))
    }
}
