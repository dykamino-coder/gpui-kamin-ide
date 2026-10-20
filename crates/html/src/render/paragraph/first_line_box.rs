//! Root inline metrics of the first formatted line (CSS Pseudo 4 §2.1.2).

use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::paragraph::Paragraph;

pub(super) fn configure(para: Paragraph, block: &Computed, opts: &RenderOpts) -> Paragraph {
    let Some(first) = block.first_line.as_deref() else {
        return para;
    };
    let normal = super::own_size(block, opts);
    let size = match first.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => normal * k,
        _ => normal,
    };
    let height = match first.line_height.or(block.line_height) {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => size * k,
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
