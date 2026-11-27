//! Сборка backend-списка glyph fallback из вычисленных семейств; не first available font.

use crate::style::computed::Computed;

pub(crate) fn fallbacks(
    style: &Computed,
    base: Option<gpui::FontFallbacks>,
    vcx: crate::values::ValueCx<'_>,
) -> Option<gpui::FontFallbacks> {
    let Some(families) = &style.inh.font.font_families else {
        return base.or_else(crate::text::fonts::document_fallbacks);
    };
    // CSS Fonts 4 section 5.2: missing glyphs continue through the authored
    // family list before system fallback. Metrics still use the first available
    // font; DirectWrite maps only characters absent from that primary font.
    // A family made of several `@font-face` rules is one composite font
    // (section 4.5): its other subsets follow the primary face, each limited
    // to its own `unicode-range`, before the next family of the list.
    let primary = style.inh.font.font_family.as_deref();
    let mut list: Vec<String> = primary
        .filter(|name| !name.is_empty())
        .map(|name| composite(name, style, vcx).into_iter().skip(1).collect())
        .unwrap_or_default();
    for name in families
        .iter()
        .filter(|name| Some(name.as_str()) != primary)
    {
        let faces = composite(name, style, vcx);
        if !faces.is_empty() {
            list.extend(faces);
        } else if crate::text::metrics::font_installed(name, vcx) {
            list.push(name.clone());
        }
    }
    if let Some(document) = crate::text::fonts::document_fallbacks() {
        list.extend(document.fallback_list().iter().cloned());
    }
    (!list.is_empty()).then(|| gpui::FontFallbacks::from_fonts(list))
}

/// The backend entries of `name`'s composite face, the leading face first:
/// a subset with a `unicode-range` serves only those code points.
fn composite(name: &str, style: &Computed, vcx: crate::values::ValueCx<'_>) -> Vec<String> {
    vcx.doc
        .font_snapshot()
        .composite_for(name, style.font_request(name, 0.0))
        .into_iter()
        .map(|(family, ranges)| {
            if ranges.is_empty() {
                family
            } else {
                gpui::FontFallbacks::restricted(&family, &ranges)
            }
        })
        .collect()
}
