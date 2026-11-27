//! Шрифт, который запрашивает вычисленный стиль: семейство (с алиасами `@font-face` и
//! общими запасными), насыщенность, начертание и возможности — в том виде, в каком его
//! запрашивают измерение и шейпинг текста. Внутри сборки документа результат общий для
//! стилей с теми же унаследованными группами (`RunFaces`).

use crate::layout::cx::run_faces::Role;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::values::ValueCx;

pub(crate) fn measure_font(c: &Computed, opts: &RenderOpts) -> gpui::Font {
    let (base, vcx) = (opts.text.font(), opts.cx.vcx());
    match opts.cx.layout().run_faces.active() {
        // Кегль базы в вывод не входит: в ключе он постоянный.
        Some(memo) => {
            memo.face(Role::Measure, c, &base, 0.0, vcx, || {
                (derive(c, base.clone(), vcx), None)
            })
            .0
        }
        None => derive(c, base, vcx),
    }
}

fn derive(c: &Computed, mut font: gpui::Font, vcx: ValueCx<'_>) -> gpui::Font {
    font.fallbacks = crate::text::fonts::fallbacks::fallbacks(c, font.fallbacks, vcx);
    if let Some(family) = c.inh.font.font_family.as_ref().filter(|f| !f.is_empty()) {
        font.family = crate::text::fonts::alias_for(family, c, vcx)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if c.inh.font.monospace == Some(true) {
        font.family =
            crate::text::metrics::mono_family_for(c.inh.text_flow.lang.as_deref(), vcx).into();
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
