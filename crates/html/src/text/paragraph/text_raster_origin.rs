//! Привязка глифов и подложек прогонов к сетке устройства при отрисовке:
//! подложка строчной коробки рисуется от точного места раскладки в рамке
//! непрозрачной заливки предка (CSS 2.1 §10.6.1, §14.2); базовая линия
//! сглаженных глифов округляется к ближайшей строке устройства ровно один
//! раз (§10.8.1 — базовая определена до растеризации), шрифт с точными
//! глифами (Ahem) и повёрнутый текст остаются на дробном месте.

use super::*;
use gpui::{App, Pixels, Point, Window, point, px};

impl Paragraph {
    pub(crate) fn opaque_background(mut self, c: &crate::style::computed::Computed) -> Self {
        let empty = |v| matches!(v, Some(crate::style::values::value::Len::Px(n) | crate::style::values::value::Len::Pct(n) | crate::style::values::value::Len::Em(n)) if n == 0.0);
        self.opaque_text_origin = c.background.is_some_and(|color| color.a == 1.0)
            && c.bg_clip != Some(crate::style::computed::BgClip::Text)
            && !empty(c.width)
            && !empty(c.height);
        self
    }

    pub(super) fn paint_run_background(
        &self,
        shaped: &gpui::ShapedLine,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // CSS 2.1 section 10.6.1: an inline content box is independent of
        // line-height. Restore its layout origin before GPUI adds half-leading;
        // snapping the paragraph first shifts equivalent baseline-aligned boxes.
        // Glyph painting restores this same offset through Window separately.
        let mut exact = origin + self.glyph_nudge;
        if self.selection_vertical.is_none()
            && !self.ruby_rotated
            && !self.paint_context.is_rotated()
        {
            let base = (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
            // CSS 2.1 section 14.2: transparent descendants paint in their
            // containing opaque fill's device frame, as glyphs do below.
            let offset = window
                .css_text_background_offset(exact + point(px(0.0), base))
                .or(self.opaque_text_origin.then_some(self.glyph_nudge));
            exact -= offset.unwrap_or_default();
        }
        let _ = shaped.paint_background(
            exact,
            self.line_height,
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        );
    }

    pub(super) fn text_raster_origin(
        &self,
        shaped: &gpui::ShapedLine,
        origin: Point<Pixels>,
        window: &Window,
    ) -> Point<Pixels> {
        if self.selection_vertical.is_some() || self.ruby_rotated || self.paint_context.is_rotated()
        {
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
        let base = (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        self.raster_origin_for_baseline(origin, base, window)
    }

    /// Device-pixel origin for antialiased glyphs whose baseline lies `base`
    /// below `origin` (a line of this paragraph, or an emphasis mark's own
    /// annotation line).
    pub(super) fn raster_origin_for_baseline(
        &self,
        origin: Point<Pixels>,
        base: Pixels,
        window: &Window,
    ) -> Point<Pixels> {
        // Ruby bases/annotations share the vertical line's glyph raster frame.
        // Rounding their flat Y baseline first would round the physical X
        // axis before the text transform, unlike native emphasis glyphs.
        if self.selection_vertical.is_some() || self.ruby_rotated || self.paint_context.is_rotated()
        {
            return origin;
        }
        // A transparent descendant paints over its ancestor's fill (CSS 2.1
        // section 14.2): keep its horizontal position within the device
        // frame of the paragraph's own opaque fill, else of that ancestor
        // fill, so text in a pixel-snapped box does not drift across glyph
        // subpixel variants (`position-sticky-table-pixel-rounding`,
        // `flexbox-baseline-multi-item-horiz-001a`). The baseline row is NOT
        // taken from the fill: Blink draws glyphs at their fractional layout
        // baseline and Skia rounds Y on its own (no vertical subpixel
        // variants), so equal baselines land on one device row whether or
        // not an ancestor fill was snapped up or down (`multicol-span-all-
        // children-height-004a`, `box-generation-001` at device scale 1).
        let baseline = origin + self.glyph_nudge + point(px(0.0), base);
        let background_offset = self
            .opaque_text_origin
            .then_some(self.glyph_nudge)
            .or_else(|| window.css_text_background_offset(baseline))
            .unwrap_or_default();
        let origin = origin - point(background_offset.x, px(0.0));
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
