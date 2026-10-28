//! Срез строк `text-box-trim` у разрывов колонок (css-inline-3 §4.2; csswg-drafts#5335):
//! строка до разрыва кончается на своей метрике края, строка после — с неё начинается.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;

/// `(срез начала строки у начала фрагмента, срез конца строки у его конца)` для
/// копии из одних строк. Сторону решает ближайшая коробка со срезом этой стороны:
/// многоколоночник режет у каждой своей колонки, коробка-копия — только при
/// `box-decoration-break: clone` (каждый её фрагмент — целая коробка), при `slice`
/// — лишь у своих первой и последней строки (`text-box-trim-multicol-004/005`).
/// Анонимный блок строк — строки самого хоста.
pub(super) fn break_trim(
    copy: &Element,
    host: &Computed,
    inner: &BoxStyle,
    opts: &RenderOpts,
) -> (f32, f32) {
    if !crate::layout::fragment::line_shape::inline_content(copy) {
        return (0.0, 0.0);
    }
    let own = copy.tag != "anon-block";
    let s = copy.style.used();
    let side = |mine: bool, host: bool| if own && mine { s.bdb_clone } else { host };
    let start = side(s.text_box_trim_start, host.text_box_trim_start);
    let end = side(s.text_box_trim_end, host.text_box_trim_end);
    if !start && !end {
        return (0.0, 0.0);
    }
    let Some((size, lh)) = line_metrics(inner, opts) else {
        return (0.0, 0.0);
    };
    let amount = |start: bool| trim_amount(inner, size, lh, start, opts);
    (
        if start { amount(true) } else { 0.0 },
        if end { amount(false) } else { 0.0 },
    )
}

/// Кегль и высота строки коробки строк в точках.
fn line_metrics(inner: &BoxStyle, opts: &RenderOpts) -> Option<(f32, f32)> {
    let size = match inner.font_size {
        Some(Len::Px(v)) if v > 0.0 => v,
        None => opts.base_size(),
        _ => return None,
    };
    let lh = match inner.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        None => crate::text::metrics::normal_px(
            crate::text::text_box::normal_fraction(inner, opts),
            size,
            opts.cx.text_fonts(),
        ),
        _ => return None,
    };
    (lh > 0.0).then_some((size, lh))
}

/// Срез с одной стороны строки: полулидинг плюс расстояние от подъёма/спуска до
/// метрики края (та же арифметика, что у срезов первой и последней строки).
fn trim_amount(s: &Computed, size: f32, lh: f32, start: bool, opts: &RenderOpts) -> f32 {
    let family = s.font_family.clone().unwrap_or_default();
    let (ascent, descent, cap) =
        crate::text::metrics::vmetrics_px(&family, size, opts.cx.text_fonts(), opts.cx.vcx());
    let half = (lh - (ascent + descent)) / 2.0;
    let edge = crate::text::text_box::edge_inset(
        s,
        &family,
        size,
        (ascent, descent, cap),
        start,
        opts.cx.vcx(),
    );
    (half + edge).max(0.0)
}
