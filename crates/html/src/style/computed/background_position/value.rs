//! Computed background-position expression and its cascade-only font resolution.

use crate::style::computed::resolve::image_lengths::lengths;
use crate::style::values::value::LengthExpression;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PositionExpression {
    expression: LengthExpression,
    pending: Option<String>,
    from_end: bool,
}

impl PositionExpression {
    pub(super) fn parse(raw: &str) -> Option<Self> {
        let low = raw.to_ascii_lowercase();
        if !low.contains('%')
            || !["min(", "max(", "clamp("]
                .iter()
                .any(|name| low.contains(name))
        {
            return None;
        }
        let typed = lengths(raw, 16.0, 16.0, 8.0, 8.0, 16.0);
        let expression = LengthExpression::parse(&typed)?;
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

    pub(crate) fn needs_font_resolution(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn resolve_font(&mut self, font: f32, rem: f32, ex: f32, ch: f32, line: f32) {
        if let Some(raw) = self.pending.take()
            && let Some(expression) =
                LengthExpression::parse(&lengths(&raw, font, rem, ex, ch, line))
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
