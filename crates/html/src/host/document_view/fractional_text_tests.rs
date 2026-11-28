//! Синтетический текстовый backend для геометрического теста сходимости.
use gpui::{
    Bounds, DevicePixels, Font, FontId, FontMetrics, FontRun, GlyphId, LineLayout, NoopTextSystem,
    Pixels, PlatformTextSystem, RenderGlyphParams, Size, TextRenderingMode, point, size,
};
use std::borrow::Cow;

// Детерминированные дробные метрики: тест не зависит от установленных шрифтов.
pub(super) struct FractionalTextSystem;
impl PlatformTextSystem for FractionalTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> gpui::Result<()> {
        NoopTextSystem.add_fonts(fonts)
    }
    fn all_font_names(&self) -> Vec<String> {
        NoopTextSystem.all_font_names()
    }
    fn font_id(&self, font: &Font) -> gpui::Result<FontId> {
        NoopTextSystem.font_id(font)
    }
    fn font_metrics(&self, id: FontId) -> FontMetrics {
        {
            let mut metrics = NoopTextSystem.font_metrics(id);
            metrics.ascent = 950.;
            metrics.descent = -250.;
            metrics.cap_height = 700.;
            metrics.x_height = 536.;
            metrics
        }
    }
    fn typographic_bounds(&self, font: FontId, glyph: GlyphId) -> gpui::Result<Bounds<f32>> {
        NoopTextSystem.typographic_bounds(font, glyph)
    }
    fn advance(&self, font: FontId, glyph: GlyphId) -> gpui::Result<Size<f32>> {
        {
            let _ = font;
            Ok(size(advance(char::from_u32(glyph.0).unwrap_or(' ')), 0.))
        }
    }
    fn glyph_for_char(&self, font: FontId, ch: char) -> Option<GlyphId> {
        {
            let _ = font;
            Some(GlyphId(ch as u32))
        }
    }
    fn glyph_raster_bounds(&self, _: &RenderGlyphParams) -> gpui::Result<Bounds<DevicePixels>> {
        Ok(Bounds {
            origin: point(DevicePixels(0), DevicePixels(0)),
            size: size(DevicePixels(1), DevicePixels(1)),
        })
    }
    fn rasterize_glyph(
        &self,
        _: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> gpui::Result<(Size<DevicePixels>, Vec<u8>)> {
        Ok((bounds.size, vec![255]))
    }
    fn layout_line(&self, text: &str, size: Pixels, runs: &[FontRun]) -> LineLayout {
        {
            let mut line = NoopTextSystem.layout_line(text, size, runs);
            let mut x = gpui::px(0.);
            for (glyph, ch) in line
                .runs
                .iter_mut()
                .flat_map(|run| &mut run.glyphs)
                .zip(text.chars())
            {
                glyph.position.x = x;
                x += size * (advance(ch) / 1000.);
            }
            line.width = x;
            line.ascent = size * 0.95;
            line.descent = size * 0.25;
            line
        }
    }
    fn recommended_rendering_mode(&self, font: FontId, size: Pixels) -> TextRenderingMode {
        NoopTextSystem.recommended_rendering_mode(font, size)
    }
}

fn advance(ch: char) -> f32 {
    match ch {
        'F' | 's' | 'z' => 550.,
        'l' | 'i' => 350.,
        ' ' => 300.,
        'c' | 't' => 500.,
        'm' => 850.,
        _ => 600.,
    }
}
