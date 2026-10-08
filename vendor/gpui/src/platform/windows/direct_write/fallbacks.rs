//! Compose fallback mappings for mixed-family DirectWrite text layouts.

use super::*;

impl DirectWriteState {
    pub(super) fn generate_font_fallbacks(
        &self,
        fallbacks: &FontFallbacks,
        base_family: Option<&str>,
    ) -> Result<Option<IDWriteFontFallback>> {
        if fallbacks.fallback_list().is_empty() {
            return Ok(None);
        }
        unsafe {
            let builder = self.components.factory.CreateFontFallbackBuilder()?;
            let base_family = base_family.map(HSTRING::from);
            let font_set = &self.system_font_collection.GetFontSet()?;
            for entry in fallbacks.fallback_list() {
                // KaminIDE patch: запись `Семейство@3000-30FF,31F0-31FF`
                // ограничивает подстановку семейства этими диапазонами
                // (`FontFallbacks::restricted`) — документ просит шрифт
                // только под свою письменность, остальное по-прежнему идёт
                // системной подстановкой. Запись без `@` — как раньше.
                let (family_name, only) = FontFallbacks::split_restricted(entry);
                let Some(fonts) = font_set
                    .GetMatchingFonts(
                        &HSTRING::from(family_name),
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        DWRITE_FONT_STYLE_NORMAL,
                    )
                    .log_err()
                else {
                    continue;
                };
                if fonts.GetFontCount() == 0 {
                    log::error!("No matching font found for {}", family_name);
                    continue;
                }
                let font = fonts.GetFontFaceReference(0)?.CreateFontFace()?;
                let mut count = 0;
                font.GetUnicodeRanges(None, &mut count).ok();
                if count == 0 {
                    continue;
                }
                let mut unicode_ranges = vec![DWRITE_UNICODE_RANGE::default(); count as usize];
                let Some(_) = font
                    .GetUnicodeRanges(Some(&mut unicode_ranges), &mut count)
                    .log_err()
                else {
                    continue;
                };
                unicode_ranges.truncate(count as usize);
                if let Some(only) = only {
                    unicode_ranges = unicode_ranges
                        .iter()
                        .flat_map(|r| {
                            only.iter().filter_map(move |&(lo, hi)| {
                                let first = r.first.max(lo);
                                let last = r.last.min(hi);
                                (first <= last).then_some(DWRITE_UNICODE_RANGE { first, last })
                            })
                        })
                        .collect();
                    if unicode_ranges.is_empty() {
                        continue;
                    }
                }
                let target_family_name = HSTRING::from(family_name);
                builder.AddMapping(
                    &unicode_ranges,
                    &[target_family_name.as_ptr()],
                    None,
                    None,
                    base_family
                        .as_ref()
                        .map_or(PCWSTR::null(), |f| PCWSTR(f.as_ptr())),
                    1.0,
                )?;
            }
            if base_family.is_none() {
                let system_fallbacks = self.components.factory.GetSystemFontFallback()?;
                builder.AddMappings(&system_fallbacks)?;
            }
            Ok(Some(builder.CreateFontFallback()?))
        }
    }

    pub(super) fn layout_fallbacks(&self, runs: &[FontRun]) -> Result<Option<IDWriteFontFallback>> {
        let first = &self.fonts[runs[0].font_id.0];
        let mut families: Vec<&FontInfo> = Vec::new();
        for run in runs {
            let font = &self.fonts[run.font_id.0];
            if let Some(previous) = families.iter().find(|f| f.font_family == font.font_family) {
                // DirectWrite cannot distinguish two fallback lists for the same
                // base family in one layout. Preserve the existing behavior in
                // that case rather than letting one list leak into the other.
                if previous.fallback_key != font.fallback_key {
                    return Ok(first.fallbacks.clone());
                }
            } else {
                families.push(font);
            }
        }
        if families.len() < 2
            || families
                .iter()
                .all(|f| f.fallback_key == first.fallback_key)
        {
            return Ok(first.fallbacks.clone());
        }
        // CSS Fonts 4 section 5.2: each inline's family list owns its missing
        // glyphs. SetFontFallback is layout-wide, so mappings must match the
        // run's base family; system mappings come after every authored list.
        unsafe {
            let builder = self.components.factory.CreateFontFallbackBuilder()?;
            for font in families {
                if let Some(fallbacks) = &font.scoped_fallbacks {
                    builder.AddMappings(fallbacks)?;
                }
            }
            builder.AddMappings(&self.components.factory.GetSystemFontFallback()?)?;
            Ok(Some(builder.CreateFontFallback()?))
        }
    }
}
