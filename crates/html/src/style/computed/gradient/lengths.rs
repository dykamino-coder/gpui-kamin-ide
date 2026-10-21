//! Разрешает разобранные длины градиента после наследования шрифта, до отрисовки.

use super::{Computed, Gradient, GradientShape, Len, RadialSize};
use crate::document::state::ValueCx;
use crate::style::values::value::calc_get;

impl Gradient {
    pub(crate) fn needs_line_height(&self, vcx: ValueCx<'_>) -> bool {
        let needs_line = |length: &Option<Len>| length.is_some_and(|l| l.needs_line_height(vcx));
        self.stop_lengths.iter().any(needs_line)
            || match &self.shape {
                GradientShape::Radial { position, size, .. } => {
                    needs_line(&position.x)
                        || needs_line(&position.y)
                        || match size {
                            RadialSize::Explicit(first, second) => {
                                needs_line(&Some(*first)) || needs_line(second)
                            }
                            _ => false,
                        }
                }
                _ => false,
            }
    }

    pub(crate) fn resolve_lengths(&mut self, fix: impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) {
        if !self.stop_lengths.is_empty() {
            for (length, stop) in self.stop_lengths.iter_mut().zip(&mut self.stops_raw) {
                fix(length);
                let terms = match *length {
                    Some(Len::Px(px)) => Some((0.0, px)),
                    Some(Len::Pct(pct)) => Some((pct, 0.0)),
                    Some(Len::Calc(id)) => calc_get(id, vcx).length_percentage(),
                    _ => None,
                };
                if let Some((pct, px)) = terms {
                    stop.1 = (pct != 0.0 || matches!(length, Some(Len::Pct(_) | Len::Calc(_))))
                        .then_some(pct);
                    stop.2 = (px != 0.0 || pct == 0.0).then_some(px);
                    *length = None;
                }
            }
            self.stops = crate::style::computed::place_stops(
                self.stops_raw
                    .iter()
                    .map(|(color, pct, _)| (*color, *pct))
                    .collect(),
            );
            self.stops_px = if self
                .stops_raw
                .iter()
                .all(|(_, pct, px)| pct.is_none() && px.is_some())
            {
                self.stops_raw
                    .iter()
                    .map(|(color, _, px)| (*color, px.unwrap_or(0.0)))
                    .collect()
            } else {
                vec![]
            };
            if self.stop_lengths.iter().all(Option::is_none) {
                self.stop_lengths.clear();
            }
        }
        if let GradientShape::Radial { position, size, .. } = &mut self.shape {
            fix(&mut position.x);
            fix(&mut position.y);
            if let RadialSize::Explicit(first, second) = size {
                let mut length = Some(*first);
                fix(&mut length);
                if let Some(length) = length {
                    *first = length;
                }
                fix(second);
            }
        }
    }
}

impl Computed {
    pub(crate) fn resolve_gradient_lengths(
        &mut self,
        fix: impl Fn(&mut Option<Len>),
        vcx: ValueCx<'_>,
    ) {
        let uses_line = self.bg_lists.needs_line_height(vcx)
            || self.image_operands().any(|i| i.needs_line_height(vcx))
            || [self.gradient.as_ref(), self.gradient_current.as_ref()]
                .into_iter()
                .flatten()
                .any(|g| g.needs_line_height(vcx));
        let line = if uses_line {
            self.image_font_metrics(vcx).3
        } else {
            0.0
        };
        let fix = |length: &mut Option<Len>| {
            fix(length);
            if uses_line && let Some(length) = length {
                length.resolve_line_height(line, vcx);
            }
        };
        for gradient in [self.gradient.as_mut(), self.gradient_current.as_mut()]
            .into_iter()
            .flatten()
        {
            gradient.resolve_lengths(&fix, vcx);
        }
        // Layer positioning and size share the same computed length closure.
        self.bg_lists.resolve_lengths(&fix, vcx);
        for image in self.image_operands() {
            image.resolve_lengths(&fix, vcx);
        }
    }
}

#[cfg(test)]
mod tests;
