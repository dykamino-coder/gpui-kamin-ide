//! CSS Fonts 5 font-size-adjust: each fallback face of a normalized run is
//! selected before shaping and pinned as its own format range at its used size.
//!
//! DirectWrite applies one font fallback to the whole layout and keys it by
//! the base family, so two runs of one family cannot carry different
//! normalizations through it. Selecting the faces here makes each pinned
//! range a shaping boundary of its own, as with any change of face or size.

use super::*;

/// A UTF-16 range of one run that asks for metric normalization.
pub(super) struct AdjustedRun {
    pub(super) start: u32,
    pub(super) len: u32,
    pub(super) font: FontId,
    pub(super) size: f32,
}

/// The line text as analysis input for face selection.
#[implement(IDWriteTextAnalysisSource)]
struct Source {
    text: Vec<u16>,
    locale: HSTRING,
}

#[allow(non_snake_case)]
impl IDWriteTextAnalysisSource_Impl for Source_Impl {
    fn GetTextAtPosition(
        &self,
        position: u32,
        text: *mut *mut u16,
        length: *mut u32,
    ) -> windows::core::Result<()> {
        let rest = self.text.get(position as usize..).unwrap_or(&[]);
        // DirectWrite supplies valid out pointers; the text outlives the call.
        unsafe {
            *text = if rest.is_empty() {
                std::ptr::null_mut()
            } else {
                rest.as_ptr() as *mut u16
            };
            *length = rest.len() as u32;
        }
        Ok(())
    }

    fn GetTextBeforePosition(
        &self,
        position: u32,
        text: *mut *mut u16,
        length: *mut u32,
    ) -> windows::core::Result<()> {
        let head = &self.text[..(position as usize).min(self.text.len())];
        unsafe {
            *text = if head.is_empty() {
                std::ptr::null_mut()
            } else {
                head.as_ptr() as *mut u16
            };
            *length = head.len() as u32;
        }
        Ok(())
    }

    fn GetParagraphReadingDirection(&self) -> DWRITE_READING_DIRECTION {
        DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
    }

    fn GetLocaleName(
        &self,
        position: u32,
        length: *mut u32,
        locale: *mut *mut u16,
    ) -> windows::core::Result<()> {
        unsafe {
            *length = (self.text.len() as u32).saturating_sub(position);
            *locale = self.locale.as_ptr() as *mut u16;
        }
        Ok(())
    }

    fn GetNumberSubstitution(
        &self,
        position: u32,
        length: *mut u32,
        substitution: OutRef<'_, IDWriteNumberSubstitution>,
    ) -> windows::core::Result<()> {
        unsafe {
            *length = (self.text.len() as u32).saturating_sub(position);
        }
        substitution.write(None)
    }
}

impl DirectWriteState {
    /// Pins every fallback face that a normalized run selects. The face keeps
    /// its own weight, style and stretch; its size is `u = (m / m') s`.
    pub(super) fn pin_adjusted_faces(
        &self,
        components: &DirectWriteComponents,
        layout: &IDWriteTextLayout,
        text: &[u16],
        runs: &[AdjustedRun],
    ) -> Result<()> {
        let source: IDWriteTextAnalysisSource = Source {
            text: text.to_vec(),
            locale: components.locale.clone(),
        }
        .into();
        for run in runs {
            let info = &self.fonts[run.font.0];
            let Some(adjustment) = info
                .fallback_key
                .as_ref()
                .and_then(FontFallbacks::size_adjustment)
            else {
                continue;
            };
            if !(adjustment.aspect.is_finite() && adjustment.aspect > 0.0) {
                continue;
            }
            unsafe {
                let fallback = match &info.fallbacks {
                    Some(fallback) => fallback.clone(),
                    None => components.factory.GetSystemFontFallback()?,
                };
                let collection: IDWriteFontCollection = info.font_collection.cast()?;
                let weight = info
                    .layout_weight
                    .unwrap_or_else(|| info.font_face.GetWeight());
                let style = info.font_face.GetStyle();
                let stretch = info.font_face.GetStretch();
                let end = run.start + run.len;
                let mut at = run.start;
                while at < end {
                    let mut mapped = 0;
                    let mut font = None;
                    let mut scale = 1.0;
                    fallback.MapCharacters(
                        &source,
                        at,
                        end - at,
                        &collection,
                        PCWSTR(info.font_family_h.as_ptr()),
                        weight,
                        style,
                        stretch,
                        &mut mapped,
                        &mut font,
                        &mut scale,
                    )?;
                    let mapped = mapped.clamp(1, end - at);
                    if let Some(font) = font {
                        let face = font.CreateFontFace()?;
                        let primary: IDWriteFontFace5 = info.font_face.cast()?;
                        if !primary.Equals(&face).as_bool() {
                            let aspect = aspect(&font, adjustment.metric)?;
                            if aspect > 0.0 {
                                let range = DWRITE_TEXT_RANGE {
                                    startPosition: at,
                                    length: mapped,
                                };
                                pin(layout, &font, run.size * (adjustment.aspect / aspect), range)?;
                            }
                        }
                    }
                    at += mapped;
                }
            }
        }
        Ok(())
    }
}

/// The selected face as an explicit range: DirectWrite resolves the same
/// family, weight, style and stretch in the face's own collection.
unsafe fn pin(
    layout: &IDWriteTextLayout,
    font: &IDWriteFont,
    size: f32,
    range: DWRITE_TEXT_RANGE,
) -> windows::core::Result<()> {
    unsafe {
        let family = font.GetFontFamily()?;
        let names = family.GetFamilyNames()?;
        let mut index = 0;
        let mut exists = BOOL(0);
        names.FindLocaleName(w!("en-us"), &mut index, &mut exists)?;
        if !exists.as_bool() {
            index = 0;
        }
        let mut name = vec![0u16; names.GetStringLength(index)? as usize + 1];
        names.GetString(index, &mut name)?;
        layout.SetFontCollection(&family.GetFontCollection()?, range)?;
        layout.SetFontFamilyName(PCWSTR(name.as_ptr()), range)?;
        layout.SetFontWeight(font.GetWeight(), range)?;
        layout.SetFontStyle(font.GetStyle(), range)?;
        layout.SetFontStretch(font.GetStretch(), range)?;
        layout.SetFontSize(size, range)?;
    }
    Ok(())
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
