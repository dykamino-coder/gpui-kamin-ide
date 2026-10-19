//! Resolve gradient stop colors in the element's used currentColor context.

use super::{Color, Computed, Gradient};
use crate::document::state::ValueCx;

const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

pub(crate) fn parse_gradient(raw: &str, vcx: ValueCx<'_>) -> Option<Gradient> {
    parse_gradient_with_current(raw, BLACK, vcx)
}

pub(crate) fn parse_gradient_with_current(
    raw: &str,
    current: Color,
    vcx: ValueCx<'_>,
) -> Option<Gradient> {
    super::parse::parse_gradient(raw, current, vcx)
}

/// Keep the authored expression on the style: inherited currentColor uses
/// the descendant's color (CSS Color 4 §6.4), including relative colors.
impl Computed {
    pub(crate) fn resolve_gradient_current_color(&mut self, _vcx: ValueCx<'_>) {
        let current = self.color.unwrap_or(BLACK);
        for image in self.image_operands() {
            image.resolve_colors(current);
        }
        if (self.gradient.is_some()
            || self.background_all_inherit
            || self.inherit_bits & super::inh::BG_IMAGE != 0)
            && let Some(template) = self.gradient_current.as_ref()
        {
            let mut gradient = self.gradient.clone().unwrap_or_else(|| template.clone());
            gradient.resolve_colors(self.color.unwrap_or(BLACK));
            self.gradient = Some(gradient);
        }
    }
}

impl Gradient {
    pub(crate) fn resolve_colors(&mut self, current: Color) {
        if self.colors.is_empty() {
            return;
        }
        for (index, expression) in self.colors.iter().enumerate() {
            let color = expression.resolve_for_interpolation(current);
            if let Some(stop) = self.stops.get_mut(index) {
                stop.0 = color;
            }
            if let Some(stop) = self.stops_px.get_mut(index) {
                stop.0 = color;
            }
            if let Some(stop) = self.stops_raw.get_mut(index) {
                stop.0 = color;
            }
        }
        if let Some(first) = self.stops.first() {
            self.from = first.0;
        }
        if let Some(last) = self.stops.last() {
            self.to = last.0;
        }
    }
}

#[cfg(test)]
mod tests;
