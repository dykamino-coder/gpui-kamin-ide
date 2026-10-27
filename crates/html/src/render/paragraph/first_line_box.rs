//! Сборка абзаца: метрики корневой строчной коробки первой форматированной
//! строки при `::first-line` (CSS Pseudo 4 §2.1.2). Из слоя псевдоэлемента
//! берутся шрифт стойки, кегль и высота строки; их получает `Paragraph`.

use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::paragraph::Paragraph;

pub(super) fn configure(para: Paragraph, block: &Computed, opts: &RenderOpts) -> Paragraph {
    let Some(first) = block.first_line.as_deref() else {
        return para;
    };
    let normal = match block.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let size = first.used_font(normal, opts.cx.vcx()).size_px;
    let height = match first.line_height.or(block.line_height) {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => first
            .numeric_line_value_px(Some(Len::Pct(k)), normal)
            .unwrap_or(size),
        _ => crate::text::metrics::normal_px(
            crate::text::text_box::normal_fraction(first, opts),
            size,
            opts.cx.vcx(),
        ),
    };
    para.first_line_box(
        crate::text::inline::strut_font(first, &opts.text, opts.cx.vcx()),
        gpui::px(size),
        height,
    )
}
