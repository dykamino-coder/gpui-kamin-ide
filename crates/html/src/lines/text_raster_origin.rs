//! Preserve exact glyph baselines and align platform raster baselines at paint time.

use super::*;

impl Paragraph {
    pub(crate) fn opaque_background(mut self, c: &crate::computed::Computed) -> Self {
        let empty = |v| matches!(v, Some(crate::value::Len::Px(n) | crate::value::Len::Pct(n) | crate::value::Len::Em(n)) if n == 0.0);
        self.opaque_text_origin = c.background.is_some_and(|color| color.a == 1.0)
            && c.bg_clip != Some(crate::computed::BgClip::Text)
            && !empty(c.width)
            && !empty(c.height);
        self
    }

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
            .all(|run| window.text_system().pixel_exact_glyphs(run.font_id))
        {
            return origin;
        }
        // Keep the raster origin coherent with an opaque box's snapped fill
        // (CSS 2.1 section 14.2), without clipping glyph overhang or changing
        // layout. Pixel-exact glyphs retain their own shared edge grid above.
        let base = (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        // A transparent descendant paints over its ancestor's fill (CSS 2.1
        // section 14.2). Keep its exact relative position within that fill's
        // device frame instead of independently snapping every paragraph.
        let baseline = origin + self.glyph_nudge + point(px(0.0), base);
        let background_offset = window
            .css_text_background_offset(baseline)
            .or(self.opaque_text_origin.then_some(self.glyph_nudge));
        let origin = origin - background_offset.unwrap_or_default();
        // GPUI has no vertical subpixel variants on Windows/Linux; flooring the
        // baseline biases fractional half-leading upward. CSS 2.1 section 10.8.1
        // defines the baseline before rasterization, so preserve it in layout
        // and choose its nearest device pixel only for painting.
        let scale = window.scale_factor();
        // Window::paint_glyph adds the exact paragraph offset afterwards;
        // round the final baseline, including that offset, only once.
        let y = f32::from(origin.y + self.glyph_nudge.y + base) * scale;
        let target = y.round();
        let mut paint_y = origin.y + px((target - y) / scale);
        // ShapedLine adds half-leading first, then Window adds the paragraph
        // offset. f32 reconstruction can produce N-epsilon for our chosen N;
        // the backend floors Y, selecting the preceding row. Recover that N
        // in the actual paint order without moving the layout baseline.
        let actual = f32::from(paint_y + base + self.glyph_nudge.y) * scale;
        if actual < target {
            paint_y += px((target - actual) / scale);
            paint_y = px(f32::from(paint_y).next_up());
        }
        point(origin.x, paint_y)
    }
}
