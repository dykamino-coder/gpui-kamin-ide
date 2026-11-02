//! Вычисленный стиль, градиенты: разрешение `currentcolor` и относительных
//! цветов (`rgb(from currentcolor …)`) в стопах по цвету САМОГО элемента.
//! Авторское выражение остаётся на стиле: унаследованный `currentcolor` берёт
//! цвет потомка (CSS Color 4 §6.4). Подставляются только цвета стопов;
//! направления и позиции не трогаются.

use super::{Color, Computed, Gradient};
use crate::values::ValueCx;

const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

pub fn parse_gradient(raw: &str, vcx: ValueCx<'_>) -> Option<Gradient> {
    parse_gradient_with_current(raw, BLACK, vcx)
}

pub fn parse_gradient_with_current(
    raw: &str,
    current: Color,
    vcx: ValueCx<'_>,
) -> Option<Gradient> {
    super::parse::parse_gradient(raw, current, vcx)
}

/// Keep the authored expression on the style: inherited currentColor uses
/// the descendant's color (CSS Color 4 §6.4), including relative colors.
impl Computed {
    pub fn resolve_gradient_current_color(&mut self, _vcx: ValueCx<'_>) {
        let current = self.inh.ui.color.unwrap_or(BLACK);
        for image in self.image_operands() {
            image.resolve_colors(current);
        }
        if (self.reset_background.gradient.is_some()
            || self
                .reset_cascade
                .inherit_longhands
                .contains(crate::style::css::LonghandId::BackgroundImage))
            && let Some(template) = self.reset_background.gradient_current.as_ref()
        {
            let mut gradient = self
                .reset_background
                .gradient
                .clone()
                .unwrap_or_else(|| template.clone());
            gradient.resolve_colors(self.inh.ui.color.unwrap_or(BLACK));
            self.reset_background.gradient = Some(gradient);
        }
    }
}

impl Gradient {
    pub fn resolve_colors(&mut self, current: Color) {
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
