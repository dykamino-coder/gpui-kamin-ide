//! Сборка backend-списка glyph fallback из вычисленных семейств; не first available font.

use crate::style::computed::Computed;

pub(crate) fn fallbacks(
    style: &Computed,
    base: Option<gpui::FontFallbacks>,
    vcx: crate::values::ValueCx<'_>,
) -> Option<gpui::FontFallbacks> {
    // The rest of the primary's composite face comes first, each part only
    // for its `unicode-range` (css-fonts-4 §5.1 step 3).
    let parts: Vec<String> = style
        .inh
        .font
        .font_family
        .as_deref()
        .map(|family| {
            vcx.doc
                .font_snapshot()
                .composite_rest(family, style.font_request(family, 0.0))
        })
        .unwrap_or_default()
        .into_iter()
        .map(|(face, ranges)| {
            if ranges.is_empty() {
                face
            } else {
                gpui::FontFallbacks::restricted(&face, &ranges)
            }
        })
        .collect();
    let Some(families) = &style.inh.font.font_families else {
        if parts.is_empty() {
            return base.or_else(crate::text::fonts::document_fallbacks);
        }
        let mut list = parts;
        if let Some(rest) = base.or_else(crate::text::fonts::document_fallbacks) {
            list.extend(rest.fallback_list().iter().cloned());
        }
        return Some(gpui::FontFallbacks::from_fonts(list));
    };
    // CSS Fonts 4 section 5.2: missing glyphs continue through the authored
    // family list before system fallback. Metrics still use the first available
    // font; DirectWrite maps only characters absent from that primary font.
    let mut list: Vec<String> = parts;
    list.extend(
        families
            .iter()
            .filter(|name| Some(name.as_str()) != style.inh.font.font_family.as_deref())
            .filter(|name| {
                crate::text::metrics::font_installed(name, vcx)
                    || crate::text::fonts::alias(name, vcx).is_some()
            })
            .map(|name| {
                crate::text::fonts::alias_for(name, style, vcx).unwrap_or_else(|| name.clone())
            }),
    );
    if let Some(document) = crate::text::fonts::document_fallbacks() {
        list.extend(document.fallback_list().iter().cloned());
    }
    (!list.is_empty()).then(|| gpui::FontFallbacks::from_fonts(list))
}
