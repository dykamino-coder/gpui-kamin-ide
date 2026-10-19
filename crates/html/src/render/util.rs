//! Помощники сборки: шрифт замера, ключи текста, тени и доступ к тексту поддерева.

use crate::dom::Node;
use crate::render::*;
use crate::style::computed::Computed;

pub(crate) fn measure_font(c: &Computed, opts: &RenderOpts) -> gpui::Font {
    let mut font = opts.text.font();
    font.fallbacks = crate::style::computed::font_family::fallbacks(c, font.fallbacks);
    if let Some(family) = c.font_family.as_ref().filter(|f| !f.is_empty()) {
        font.family = crate::text::fonts::alias_for(family, c)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if c.monospace == Some(true) {
        font.family = crate::text::metrics::mono_family_for(c.lang.as_deref()).into();
    }
    if let Some(w) = c.font_weight {
        font.weight = gpui::FontWeight(w as f32);
    }
    if c.italic == Some(true) {
        font.style = gpui::FontStyle::Italic;
    }
    font.features = crate::text::fonts::with_css_weight(&font.features);
    font
}

/// Устойчивый номер абзаца для памяти выделения.
///
/// Абзац не элемент документа, своего номера у него нет; берём отпечаток его
/// текста — от кадра к кадру он не меняется, а разные абзацы почти всегда
/// различаются.
pub(super) fn text_id(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Схлопывание пробелов для тени — той же формы, что и в абзаце.
pub(crate) fn normalize_for_shadow(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.chars() {
        // Схлопываются только четыре знака CSS: идеографический и неразрывный
        // пробелы — обычные знаки со своей шириной (см. `inline.rs`).
        let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r');
        if is_space {
            if !prev_space {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        prev_space = is_space;
    }
    out
}

/// Текст поддерева — нужен формам (`<textarea>`, `<option>`).
pub fn gather_text_public(nodes: &[Node], out: &mut String) {
    gather_text(nodes, out)
}
