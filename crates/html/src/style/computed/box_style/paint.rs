//! Именованные запросы результата оформления; наследование читает CSS элемента.

use super::BoxStyle;
use crate::style::background_paint::BackgroundValues;
use crate::style::computed::{Filter, Shadow};
use crate::style::decoration_paint::ForegroundPaint;
use crate::style::values::value::{Color, ColorOrAuto, StyleColor};
use std::borrow::Cow;

impl BoxStyle {
    pub(crate) fn foreground_paint(&self) -> ForegroundPaint {
        self.paint.as_ref().map_or(
            ForegroundPaint {
                color: self.color,
                ..Default::default()
            },
            |p| p.foreground,
        )
    }

    pub(crate) fn ink(&self) -> Option<Color> {
        self.foreground_paint().color
    }

    pub(crate) fn paint_filter(&self) -> Option<Filter> {
        self.paint.as_ref().map_or(self.filter, |p| p.filter)
    }

    pub(crate) fn paint_filter_ops(&self) -> &[Filter] {
        self.paint
            .as_ref()
            .map_or(&self.scrollbar_filter_ops, |p| &p.filter_ops)
    }

    pub(crate) fn paint_background(&self) -> Cow<'_, BackgroundValues> {
        self.paint.as_ref().map_or_else(
            || Cow::Owned(BackgroundValues::from_computed(self)),
            |p| Cow::Borrowed(&p.background),
        )
    }

    pub(crate) fn gradient_as_tile(&self) -> bool {
        self.paint_background().gradient_as_tile()
    }

    pub(crate) fn current_color(&self) -> Color {
        self.ink().unwrap_or(Color::BLACK)
    }

    pub(crate) fn used_color(&self, color: &StyleColor) -> Color {
        color.resolve(self.current_color())
    }

    pub(crate) fn used_caret_color(&self) -> Option<Color> {
        match self.caret_color.as_ref().and_then(ColorOrAuto::color) {
            Some(color) => Some(self.used_color(color)),
            None => self.ink(),
        }
    }

    pub(crate) fn used_accent_color(&self) -> Option<Color> {
        self.accent_color
            .as_ref()
            .and_then(ColorOrAuto::color)
            .map(|c| self.used_color(c))
    }

    pub(crate) fn used_outline_color(&self, o: &crate::style::computed::Outline) -> Color {
        match o.color.color() {
            Some(color) => self.used_color(color),
            None if o.style == Some(2) => self.used_accent_color().unwrap_or(self.current_color()),
            None => self.current_color(),
        }
    }

    pub(crate) fn used_scrollbar_colors(&self) -> Option<[Color; 2]> {
        let colors = self.scroll.colors.as_ref()?.as_ref()?;
        Some(colors.each_ref().map(|c| self.used_color(c)))
    }

    pub(crate) fn border_shape_stroke(&self) -> (f32, Color) {
        (
            self.style_rc().border_shape_stroke().0,
            self.used_border_color(self.border_shape_side()),
        )
    }

    pub(crate) fn contour_outline_params(&self) -> Option<(f32, f32, Color)> {
        if !self.paints_decorations() {
            return None;
        }
        let (width, offset, _) = self.style_rc().contour_outline_params()?;
        Some((
            width,
            offset,
            self.used_outline_color(self.outline.as_ref()?),
        ))
    }

    pub(crate) fn shaped_outline(&self) -> Option<(f32, f32, Color)> {
        self.border_shape.as_ref()?;
        self.contour_outline_params()
    }

    pub(crate) fn used_border_color(&self, side: usize) -> Color {
        self.used_color(
            self.paint
                .as_ref()
                .map_or(&self.border_colors[side], |p| &p.border_colors[side]),
        )
    }

    pub(crate) fn used_uniform_border_color(&self) -> Option<Color> {
        let first = self.used_border_color(0);
        (1..4)
            .all(|side| self.used_border_color(side) == first)
            .then_some(first)
    }

    pub(crate) fn paint_shadows(&self) -> &[Shadow] {
        self.paint.as_ref().map_or(&self.shadows, |p| &p.shadows)
    }

    pub(crate) fn resolved_shadows(&self, inset: bool) -> Vec<(Shadow, Color)> {
        if !self.paints_decorations() {
            return Vec::new();
        }
        let shadows = if inset {
            &self.inset_shadows
        } else {
            self.paint_shadows()
        };
        shadows
            .iter()
            .map(|s| (s.clone(), s.used_color(self.current_color())))
            .collect()
    }

    pub(crate) fn border_shape_ext(&self) -> [f32; 4] {
        self.style_rc().border_shape_ext_with(
            self.paint_shadows(),
            if self.paints_decorations() {
                &self.inset_shadows
            } else {
                &[]
            },
            self.shaped_outline(),
        )
    }
}
