//! CSS Fonts 5: normalize every selected fallback face before native shaping.

use super::*;

struct Range {
    end: u32,
    adjustment: Option<FontSizeAdjustment>,
    fallback: Option<IDWriteFontFallback>,
}

#[implement(IDWriteFontFallback)]
struct AdjustedFallback {
    inner: IDWriteFontFallback,
    ranges: Vec<Range>,
}

/// Constructed only when a run requests metric normalization.
pub(super) fn for_layout(
    state: &DirectWriteState,
    components: &DirectWriteComponents,
    text: &str,
    runs: &[FontRun],
) -> Result<IDWriteFontFallback> {
    let mut at = 0;
    let mut end = 0;
    let ranges = runs
        .iter()
        .map(|run| {
            let part = text.get(at..at + run.len).unwrap_or("");
            at += run.len;
            end += part.encode_utf16().count() as u32;
            let font = &state.fonts[run.font_id.0];
            Range {
                end,
                adjustment: font
                    .fallback_key
                    .as_ref()
                    .and_then(FontFallbacks::size_adjustment),
                fallback: font.fallbacks.clone(),
            }
        })
        .collect();
    let inner = unsafe { components.factory.GetSystemFontFallback()? };
    Ok(AdjustedFallback { inner, ranges }.into())
}

#[allow(non_snake_case)]
impl IDWriteFontFallback_Impl for AdjustedFallback_Impl {
    fn MapCharacters(
        &self,
        source: Ref<'_, IDWriteTextAnalysisSource>,
        position: u32,
        length: u32,
        collection: Ref<'_, IDWriteFontCollection>,
        family: &PCWSTR,
        weight: DWRITE_FONT_WEIGHT,
        style: DWRITE_FONT_STYLE,
        stretch: DWRITE_FONT_STRETCH,
        mapped_length: *mut u32,
        mapped_font: OutRef<'_, IDWriteFont>,
        scale: *mut f32,
    ) -> windows::core::Result<()> {
        let range = self
            .ranges
            .get(self.ranges.partition_point(|range| range.end <= position));
        let length = range.map_or(length, |range| length.min(range.end - position));
        let inner = range
            .and_then(|range| range.fallback.as_ref())
            .unwrap_or(&self.inner);
        let mut font = None;
        // COM keeps the input interfaces alive; DirectWrite supplies valid out
        // pointers. A mapping must not extend into the next CSS run.
        unsafe {
            inner.MapCharacters(
                source.as_ref(),
                position,
                length,
                collection.as_ref(),
                *family,
                weight,
                style,
                stretch,
                mapped_length,
                &mut font,
                scale,
            )?;
            if let (Some(font), Some(adjustment)) =
                (font.as_ref(), range.and_then(|range| range.adjustment))
            {
                let aspect = aspect(font, adjustment.metric)?;
                if aspect > 0.0 && adjustment.aspect.is_finite() && adjustment.aspect > 0.0 {
                    // Replace the system em scale: u = (m / m') s.
                    *scale = adjustment.aspect / aspect;
                }
            }
        }
        mapped_font.write(font)
    }
}

fn aspect(font: &IDWriteFont, metric: FontSizeMetric) -> windows::core::Result<f32> {
    use FontSizeMetric::*;
    let mut metrics = DWRITE_FONT_METRICS::default();
    // Raw design units, unaffected by descriptor size-adjust or metric overrides.
    unsafe {
        font.GetMetrics(&mut metrics);
    }
    let units = f32::from(metrics.designUnitsPerEm);
    let value = match metric {
        XHeight => f32::from(metrics.xHeight),
        CapHeight => f32::from(metrics.capHeight),
        ZeroAdvance | IdeographicWidth | IdeographicHeight => {
            let face = unsafe { font.CreateFontFace()? };
            let cp = if metric == ZeroAdvance { '0' } else { '水' } as u32;
            let mut glyph = 0;
            unsafe {
                face.GetGlyphIndices(&cp, 1, &mut glyph)?;
            }
            if glyph == 0 {
                return Ok(if metric == ZeroAdvance { 0.5 } else { 1.0 });
            }
            let mut design = DWRITE_GLYPH_METRICS::default();
            unsafe {
                face.GetDesignGlyphMetrics(&glyph, 1, &mut design, false)?;
            }
            if metric == IdeographicHeight {
                design.advanceHeight as f32
            } else {
                design.advanceWidth as f32
            }
        }
    };
    Ok(if value > 0.0 && units > 0.0 {
        value / units
    } else {
        match metric {
            XHeight | ZeroAdvance => 0.5,
            CapHeight => f32::from(metrics.ascent) / units,
            IdeographicWidth | IdeographicHeight => 1.0,
        }
    })
}

impl DirectWriteState {
    pub(super) fn get_advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        unsafe {
            let font = &self.fonts[font_id.0].font_face;
            let glyph_indices = [glyph_id.0 as u16];
            let mut metrics = [DWRITE_GLYPH_METRICS::default()];
            font.GetDesignGlyphMetrics(glyph_indices.as_ptr(), 1, metrics.as_mut_ptr(), false)?;

            let metrics = &metrics[0];

            Ok(Size {
                width: metrics.advanceWidth as f32,
                height: metrics.advanceHeight as f32,
            })
        }
    }
}

#[cfg(test)]
mod tests;
