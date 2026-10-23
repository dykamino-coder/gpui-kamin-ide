//! Краска SVG: `fill`, `stroke`, `stroke-width` (css-fill-stroke-3, SVG 2
//! §13): наследуются, значение — текстом до отрисовки.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::LonghandId;
use crate::style::values::value::Len;

/// `<paint>` (css-fill-stroke-3 §2.1): `none | <color> | <url> |
/// context-fill | context-stroke`.
pub(crate) fn paint(v: &str, vcx: ValueCx<'_>) -> Option<String> {
    let lower = v.to_ascii_lowercase();
    (matches!(
        lower.as_str(),
        "none" | "context-fill" | "context-stroke" | "currentcolor"
    ) || crate::style::values::value::Color::parse(v, vcx).is_some()
        || (lower.starts_with("url(") && v.ends_with(')')))
    .then(|| v.to_string())
}

/// `stroke-width: <length-percentage [0,∞]> | <number [0,∞]>`: точки без доли
/// — числом, остальное — текстом до раскладки.
pub(crate) fn stroke_width(v: &str, vcx: ValueCx<'_>) -> Option<String> {
    let length = crate::style::computed::size_range::parse_nonnegative_size(v, vcx).filter(|l| {
        !matches!(
            l,
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
        )
    });
    let number = v.parse::<f32>().ok().filter(|n| n.is_finite() && *n >= 0.0);
    (length.is_some() || number.is_some()).then(|| {
        match crate::style::values::value::calc_pct_px(v, vcx) {
            Some((0.0, px)) => format!("{px}"),
            _ => v.to_string(),
        }
    })
}

pub(super) fn accepts(id: LonghandId, v: &str, vcx: ValueCx<'_>) -> bool {
    match id {
        LonghandId::Fill | LonghandId::Stroke => paint(v, vcx).is_some(),
        LonghandId::StrokeWidth => stroke_width(v, vcx).is_some(),
        _ => false,
    }
}

impl Computed {
    pub(super) fn apply_svg_paint(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        match id {
            LonghandId::Fill => self.svg_fill = paint(v, vcx).or(self.svg_fill.take()),
            LonghandId::Stroke => self.svg_stroke = paint(v, vcx).or(self.svg_stroke.take()),
            LonghandId::StrokeWidth => {
                self.svg_stroke_width = stroke_width(v, vcx).or(self.svg_stroke_width.take())
            }
            _ => {}
        }
    }

    pub(super) fn inherit_svg_paint(&mut self, id: LonghandId) {
        match id {
            LonghandId::Fill => self.svg_fill = None,
            LonghandId::Stroke => self.svg_stroke = None,
            LonghandId::StrokeWidth => self.svg_stroke_width = None,
            _ => {}
        }
    }
}
