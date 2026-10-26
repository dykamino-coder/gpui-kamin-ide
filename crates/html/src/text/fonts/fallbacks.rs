//! Сборка backend-списка glyph fallback из вычисленных семейств; не first available font.

use crate::style::computed::Computed;

pub(crate) fn fallbacks(
    style: &Computed,
    base: Option<gpui::FontFallbacks>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<gpui::FontFallbacks> {
    let Some(families) = &style.font_families else {
        return base.or_else(crate::text::fonts::document_fallbacks);
    };
    // CSS Fonts 4 section 5.2: missing glyphs continue through the authored
    // family list before system fallback. Metrics still use the first available
    // font; DirectWrite maps only characters absent from that primary font.
    let mut list: Vec<String> = families
        .iter()
        .filter(|name| Some(name.as_str()) != style.font_family.as_deref())
        .filter(|name| {
            crate::text::metrics::font_installed(name, vcx)
                || crate::text::fonts::alias(name, vcx).is_some()
        })
        .map(|name| crate::text::fonts::alias_for(name, style, vcx).unwrap_or_else(|| name.clone()))
        .collect();
    if let Some(document) = crate::text::fonts::document_fallbacks() {
        list.extend(document.fallback_list().iter().cloned());
    }
    (!list.is_empty()).then(|| gpui::FontFallbacks::from_fonts(list))
}
