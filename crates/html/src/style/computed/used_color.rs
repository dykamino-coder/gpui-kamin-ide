//! Используемые цвета элемента: вычисленные значения цветовых свойств
//! (`StyleColor`, `currentcolor` остаётся словом) разрешаются здесь — от
//! вычисленного `color` ЭТОГО элемента (css-color-4 §6.4). Единственное место
//! разрешения для отрисовки и раскладки.

use super::Computed;
use crate::style::values::value::{Color, ColorOrAuto, StyleColor};

impl Computed {
    /// Значение `currentcolor` элемента: его вычисленный `color`.
    pub(crate) fn current_color(&self) -> Color {
        self.color.unwrap_or(Color::BLACK)
    }

    /// Используемый цвет вычисленного значения на этом элементе.
    pub(crate) fn used_color(&self, value: &StyleColor) -> Color {
        value.resolve(self.current_color())
    }

    /// `caret-color`: `auto` — `currentcolor` (css-ui-4 §caret-color);
    /// `None` — цвета текста нет, решает тема поля.
    pub(crate) fn used_caret_color(&self) -> Option<Color> {
        match self.caret_color.as_ref().and_then(ColorOrAuto::color) {
            Some(color) => Some(self.used_color(color)),
            None => self.color,
        }
    }

    /// `accent-color`; `None` — `auto` (цвет акцента платформы).
    pub(crate) fn used_accent_color(&self) -> Option<Color> {
        let color = self.accent_color.as_ref().and_then(ColorOrAuto::color)?;
        Some(self.used_color(color))
    }

    /// `outline-color` контура `o` (css-ui-4 §outline-color): `auto` при
    /// `outline-style: auto` — акцентный цвет (`auto` акцента — цвет текста),
    /// иначе `currentcolor`.
    pub(crate) fn used_outline_color(&self, o: &super::Outline) -> Color {
        match o.color.color() {
            Some(color) => self.used_color(color),
            None if o.style == Some(2) => self.used_accent_color().unwrap_or(self.current_color()),
            None => self.current_color(),
        }
    }

    /// Используемый цвет стороны рамки `side` (top, right, bottom, left).
    pub(crate) fn used_border_color(&self, side: usize) -> Color {
        self.used_color(&self.border_colors[side])
    }

    /// Общий используемый цвет четырёх сторон рамки; `None` — цвета разные.
    pub(crate) fn used_uniform_border_color(&self) -> Option<Color> {
        let top = self.used_border_color(0);
        (1..4)
            .all(|side| self.used_border_color(side) == top)
            .then_some(top)
    }

    /// Задан ли хоть одной стороне цвет, отличный от начального `currentcolor`.
    pub(crate) fn border_color_declared(&self) -> bool {
        !self.border_colors.iter().all(StyleColor::is_current)
    }

    /// `scrollbar-color` ползунка и дорожки; `None` — `auto`.
    pub(crate) fn used_scrollbar_colors(&self) -> Option<[Color; 2]> {
        let colors = self.scroll.colors.as_ref()?.as_ref()?;
        Some(colors.each_ref().map(|c| self.used_color(c)))
    }

    /// The single derivation of the used `background` from the computed
    /// `background_color`. `current` is the element's color, known only at
    /// the merge; without it a `currentcolor`-dependent value stays unpainted.
    pub(crate) fn derive_background(&mut self, current: Option<Color>) {
        self.background = match &self.background_color {
            None => None,
            Some(StyleColor::Absolute(color)) => Some(*color),
            Some(value) => current.map(|current| value.resolve(current)),
        };
    }

    /// Set the computed `background-color` and its used value.
    pub(crate) fn set_background_color(&mut self, value: Option<StyleColor>) {
        self.background_color = value;
        self.derive_background(None);
    }

    /// Задал ли элемент свой `color` (значение или функцию от родительского).
    pub(crate) fn declares_color(&self) -> bool {
        self.color.is_some() || self.color_function.is_some()
    }
}
