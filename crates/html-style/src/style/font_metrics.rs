//! CSS-разрешение нейтральных метрик. Запасные длины выбирает style, не provider.
//! Раздельные области запроса сохраняют прежние порядок замеров и округление.

use crate::style::values::value::Len;
use crate::values::ValueCx;
use {
    kamin_font_facts::font_facts::FontMetricFacts, kamin_font_facts::font_facts::FontMetricQuery,
    kamin_font_facts::font_facts::FontRequest, kamin_font_facts::font_facts::NormalLineFacts,
};

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

pub fn horizontal(family: &str, size: f32, vcx: ValueCx<'_>) -> (f32, f32) {
    let f = facts(family, size, FontMetricQuery::Horizontal, vcx);
    (
        f.zero_advance.map_or(0.5 * size, |m| m.px),
        f.x_height.map_or(0.5 * size, |m| m.px),
    )
}

pub fn vertical(family: &str, size: f32, vcx: ValueCx<'_>) -> (f32, f32, f32) {
    let f = facts(family, size, FontMetricQuery::Vertical, vcx);
    (
        f.ascent.map_or(0.8 * size, |m| m.px),
        f.descent.map_or(0.2 * size, |m| m.px),
        f.cap_height.or(f.ascent).map_or(0.7 * size, |m| m.px),
    )
}

pub fn ideographic(family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
    facts(family, size, FontMetricQuery::Ideographic, vcx)
        .ideographic_advance
        .map_or(size, |m| m.px)
}

pub fn normal(family: &str, size: f32, vcx: ValueCx<'_>) -> NormalLineFacts {
    vcx.doc
        .font_provider()
        .normal_line(FontRequest::new(family, size), &vcx.doc.font_snapshot())
        .unwrap_or_default()
}

pub fn normal_height(family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
    let f = normal(family, size, vcx);
    if f.fraction.is_none_or(|v| v > 0.0) {
        f.height.map_or(1.2 * size, |m| m.px)
    } else {
        1.2 * size
    }
}

pub fn spacing_px(len: Option<Len>, family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
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
pub fn alphabetic(family: &str, size: f32, vcx: ValueCx<'_>) -> f32 {
    facts(family, size, FontMetricQuery::Alphabetic, vcx)
        .alphabetic_baseline
        .map_or(0.0, |m| m.px)
}
