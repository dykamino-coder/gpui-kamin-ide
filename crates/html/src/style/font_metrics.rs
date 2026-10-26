//! CSS-разрешение нейтральных метрик. Запасные длины выбирает style, не provider.
//! Раздельные области запроса сохраняют прежние порядок замеров и округление.

use crate::document::state::ValueCx;
use crate::font_facts::{FontMetricFacts, FontMetricQuery, FontRequest, NormalLineFacts};
use crate::style::values::value::Len;

fn facts(family: &str, size: f32, query: FontMetricQuery, vcx: ValueCx<'_>) -> FontMetricFacts {
    let request = FontRequest {
        metric_query: query,
        ..FontRequest::new(family, size)
    };
    vcx.doc
        .font_provider()
        .metrics(request, &vcx.doc.font_snapshot())
        .unwrap_or_default()
}

pub(crate) fn horizontal(family: &str, size: f32, vcx: ValueCx<'_>) -> (f32, f32) {
    let f = facts(family, size, FontMetricQuery::Horizontal, vcx);
    (
        f.zero_advance.map_or(0.5 * size, |m| m.px),
        f.x_height.map_or(0.5 * size, |m| m.px),
    )
}

pub(crate) fn vertical(family: &str, size: f32, vcx: ValueCx<'_>) -> (f32, f32, f32) {
    let f = facts(family, size, FontMetricQuery::Vertical, vcx);
    (
        f.ascent.map_or(0.8 * size, |m| m.px),
        f.descent.map_or(0.2 * size, |m| m.px),
        f.cap_height.or(f.ascent).map_or(0.7 * size, |m| m.px),
    )
}

pub(crate) fn ideographic(family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
    facts(family, size, FontMetricQuery::Ideographic, vcx)
        .ideographic_advance
        .map_or(size, |m| m.px)
}

pub(crate) fn normal(family: &str, size: f32, vcx: ValueCx<'_>) -> NormalLineFacts {
    vcx.doc
        .font_provider()
        .normal_line(FontRequest::new(family, size), &vcx.doc.font_snapshot())
        .unwrap_or_default()
}

pub(crate) fn normal_height(family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
    let f = normal(family, size, vcx);
    if f.fraction.is_none_or(|v| v > 0.0) {
        f.height.map_or(1.2 * size, |m| m.px)
    } else {
        1.2 * size
    }
}

pub(crate) fn adjust_aspect(family: &str, metric: u8, vcx: ValueCx<'_>) -> Option<f32> {
    // Даже cap раньше сначала прогревал горизонтальный замер.
    let (ch, ex) = horizontal(family, 1.0, vcx);
    Some(match metric {
        0 => ex,
        1 => vertical(family, 1.0, vcx).2,
        2 => ch,
        3 | 4 => ideographic(family, 1.0, vcx),
        _ => return None,
    })
}

pub(crate) fn spacing_px(len: Option<Len>, family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
    let (ch, ex) = horizontal(family, size, vcx);
    match len {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        Some(Len::Ch(k)) => k * ch,
        Some(Len::Ex(k)) => k * ex,
        Some(Len::Ic(k)) => k * ideographic(family, size, vcx),
        _ => 0.0,
    }
}

/// Базовая линия в px с прежним нулевым fallback при отсутствии BASE.
pub(crate) fn alphabetic(family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
    facts(family, size, FontMetricQuery::Alphabetic, vcx)
        .alphabetic_baseline
        .map_or(0.0, |m| m.px)
}
