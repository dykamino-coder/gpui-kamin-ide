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
        // A pixel-exact font (Ahem) is placed at its exact sub-pixel baseline
        // by GPUI itself (`Window::paint_glyph`, with the paragraph's glyph
        // offset to the unsnapped layout position); rounding this snapped
        // origin on top moved its em squares off the boxes they must match.
        if shaped
            .runs
            .iter()
            .any(|run| window.text_system().pixel_exact_glyphs(run.font_id))
        {
            return origin;
        }
        // GPUI has no vertical subpixel variants on Windows/Linux; flooring the
        // baseline biases fractional half-leading upward. CSS 2.1 section 10.8.1
        // defines the baseline before rasterization, so preserve it in layout
        // and choose its nearest device pixel only for painting.
        let base = (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        let scale = window.scale_factor();
        // Round the final baseline once, including the paragraph's fractional
        // offset from its snapped box. `Window::paint_glyph` adds that offset
        // back only for pixel-exact fonts (handled above), so for an
        // antialiased font it is part of the returned origin: otherwise the
        // rounded baseline lost the offset again, and a line of a paragraph
        // sat one device pixel off the same line in its own block
        // (`inline-block-width-002a`: `x<br>y<br>z` vs three blocks).
        let y = f32::from(origin.y + self.glyph_nudge.y + base) * scale;
        point(
            origin.x,
            origin.y + self.glyph_nudge.y + px((y.round() - y) / scale),
        )
    }
}
