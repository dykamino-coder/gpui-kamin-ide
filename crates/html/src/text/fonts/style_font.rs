//! Шрифт, который запрашивает вычисленный стиль: семейство (с алиасами `@font-face` и
//! общими запасными), насыщенность, начертание и возможности — в том виде, в каком его
//! запрашивают измерение и шейпинг текста.

use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;

pub(crate) fn measure_font(c: &Computed, opts: &RenderOpts) -> gpui::Font {
    let mut font = opts.text.font();
    font.fallbacks = crate::text::fonts::fallbacks::fallbacks(c, font.fallbacks, opts.cx.vcx());
    if let Some(family) = c.inh.font.font_family.as_ref().filter(|f| !f.is_empty()) {
        font.family = crate::text::fonts::alias_for(family, c, opts.cx.vcx())
            .unwrap_or_else(|| family.clone())
            .into();
    } else if c.inh.font.monospace == Some(true) {
        font.family =
            crate::text::metrics::mono_family_for(c.inh.text_flow.lang.as_deref(), opts.cx.vcx())
                .into();
    }
    if let Some(w) = c.inh.font.font_weight {
        font.weight = gpui::FontWeight(w as f32);
    }
    if c.inh.font.italic == Some(true) {
        font.style = gpui::FontStyle::Italic;
    }
    font.features = crate::text::fonts::with_css_weight(&font.features);
    font
}
