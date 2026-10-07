//! Preserve exact glyph baselines and align platform raster baselines at paint time.

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
        // CSS 2.1 §10.8.1 positions glyphs on the layout baseline. Pixel-exact
        // faces rasterize their outline at that fractional position on both
        // axes; snapping the baseline first moves their edges independently
        // of adjacent boxes and can open seams between consecutive lines.
        if shaped
            .runs
            .iter()
            .all(|run| window.text_system().pixel_exact_glyphs(run.font_id))
        {
            return origin;
        }
        // GPUI has no vertical subpixel variants on Windows/Linux; flooring the
        // baseline biases fractional half-leading upward. CSS 2.1 section 10.8.1
        // defines the baseline before rasterization, so preserve it in layout
        // and choose its nearest device pixel only for painting.
        let base = (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        let scale = window.scale_factor();
        // Window::paint_glyph restores this raw paragraph offset afterwards.
        // Round the final baseline once, including that fractional offset.
        let y = f32::from(origin.y + self.glyph_nudge.y + base) * scale;
        point(origin.x, origin.y + px((y.round() - y) / scale))
    }
}
