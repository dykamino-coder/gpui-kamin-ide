//! Resolve gradient stop colors in the element's used currentColor context.

use super::{Color, Computed, Gradient, split_outside_parens};
use crate::document::state::ValueCx;
use crate::style::values::color_space::resolve_relative;
use std::borrow::Cow;

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
        if let Some(image) = self
            .border_image
            .as_mut()
            .and_then(|border| border.src.as_mut())
        {
            image.resolve_colors(self.color.unwrap_or(BLACK));
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

/// Raster cache keys must distinguish the same expression on two elements.
/// Only stop colors are substituted; directions and positions stay authored.
pub(crate) fn resolve_gradient_colors<'a>(
    raw: &'a str,
    current: Color,
    vcx: ValueCx<'_>,
) -> Cow<'a, str> {
    if !(raw.contains("currentcolor") || raw.contains("from "))
        || !raw
            .split('(')
            .next()
            .is_some_and(|name| name.ends_with("gradient"))
    {
        return Cow::Borrowed(raw);
    }
    let (Some(open), Some(close)) = (raw.find('('), raw.rfind(')')) else {
        return Cow::Borrowed(raw);
    };
    let parts: Vec<String> = crate::style::css::split_args(&raw[open + 1..close])
        .into_iter()
        .map(|part| {
            let mut words = split_outside_parens(part);
            if let Some(token) = words.first_mut()
                && (token.contains("currentcolor") || token.contains("from "))
                && let Some(color) = resolve_relative(token, current, vcx)
            {
                // Relative RGB is a modern color, so preserve the default
                // OKLab interpolation rather than serializing it as rgba().
                *token = if token == "currentcolor" {
                    format!(
                        "rgba({},{},{},{})",
                        color.r * 255.0,
                        color.g * 255.0,
                        color.b * 255.0,
                        color.a
                    )
                } else {
                    format!(
                        "color(srgb {} {} {} / {})",
                        color.r, color.g, color.b, color.a
                    )
                };
            }
            words.join(" ")
        })
        .collect();
    Cow::Owned(format!("{}{})", &raw[..=open], parts.join(", ")))
}

#[cfg(test)]
mod tests;
