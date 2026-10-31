//! Вычисленное выражение background-position и его разрешение шрифта только в каскаде.

use crate::style::computed::resolve::image_lengths::lengths;
use crate::style::values::value::LengthExpression;
use crate::values::ValueCx;

#[derive(Clone, Debug, PartialEq)]
pub struct PositionExpression {
    expression: LengthExpression,
    pending: Option<String>,
    from_end: bool,
}

impl PositionExpression {
    pub(super) fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let low = raw.to_ascii_lowercase();
        if !low.contains('%')
            || !["min(", "max(", "clamp("]
                .iter()
                .any(|name| low.contains(name))
        {
            return None;
        }
        let typed = lengths(raw, 16.0, 16.0, 8.0, 8.0, 16.0);
        let expression = LengthExpression::parse(&typed, vcx)?;
        Some(Self {
            expression,
            pending: (raw.contains("lh") || crate::style::computed::has_font_units(raw))
                .then(|| raw.to_string()),
            from_end: false,
        })
    }

    pub(super) fn offset_from_end(mut self) -> Self {
        self.expression = self.expression.offset_from_end();
        self.from_end = true;
        self
    }

    pub fn needs_font_resolution(&self) -> bool {
        self.pending.is_some()
    }

    pub fn resolve_font(
        &mut self,
        font: f32,
        rem: f32,
        ex: f32,
        ch: f32,
        line: f32,
        vcx: ValueCx<'_>,
    ) {
        if let Some(raw) = self.pending.take()
            && let Some(expression) =
                LengthExpression::parse(&lengths(&raw, font, rem, ex, ch, line), vcx)
        {
            self.expression = if self.from_end {
                expression.offset_from_end()
            } else {
                expression
            };
        }
    }

    pub(super) fn evaluate(&self, basis: f32) -> Option<f32> {
        self.expression.evaluate(basis)
    }
}
