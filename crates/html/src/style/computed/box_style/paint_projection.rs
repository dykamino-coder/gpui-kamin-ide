//! Проекции собственной краски без изменения CSS, размеров и видимости якоря.

use super::BoxStyle;
use crate::style::decoration_paint::DecorationPaint;
use crate::style::values::value::Color;
use std::rc::Rc;

impl BoxStyle {
    fn own_paint_mut(&mut self) -> &mut DecorationPaint {
        let paint = self
            .paint
            .get_or_insert_with(|| Rc::new(DecorationPaint::from_computed(&self.style)));
        Rc::make_mut(paint)
    }

    pub(crate) fn paints_decorations(&self) -> bool {
        self.paint.as_ref().is_none_or(|p| p.decorations)
    }

    /// CSS2 §11.2: видимые потомки остаются видимыми, собственная краска скрыта.
    pub(crate) fn without_own_decorations(&self) -> Self {
        let mut result = self.clone();
        let paint = result.own_paint_mut();
        paint.decorations = false;
        paint.background =
            crate::style::background_paint::BackgroundValues::from_computed(&Default::default());
        paint.border_colors = std::array::from_fn(|_| {
            Color {
                a: 0.0,
                ..Color::BLACK
            }
            .into()
        });
        paint.shadows.clear();
        result
    }

    /// Фон уже входит в исходную картинку SVG-фильтра.
    pub(crate) fn without_background_color(&self) -> Self {
        let mut result = self.clone();
        result.own_paint_mut().background.color = None;
        result
    }

    /// Эффект SVG-хоста применяется к готовому снимку, а не повторно к HTML.
    pub(crate) fn without_paint_filter(&self) -> Self {
        let mut result = self.clone();
        result.own_paint_mut().filter = None;
        result
    }

    /// Поворот плоскости inline меняет порядок граней собственной краски.
    pub(crate) fn project_border_paint(&mut self, sides: [usize; 4]) {
        let paint = self.own_paint_mut();
        let colors = paint.border_colors.clone();
        paint.border_colors = sides.map(|side| colors[side].clone());
    }

    /// Цвет псевдоэлемента меняет CSS-наследование и foreground своего прогона.
    pub(crate) fn overlay_pseudo_color(&mut self, color: Option<Color>) {
        if let Some(color) = color {
            self.color = Some(color);
            self.own_paint_mut().foreground.color = Some(color);
        }
    }

    /// Слово несёт свой foreground, но не повторяет фон и рамку абзаца.
    pub(crate) fn text_paint_style(&self) -> Self {
        let mut result = Self::new(self.style.text_only());
        result.own_paint_mut().foreground = self.foreground_paint();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;

    #[test]
    fn own_suppression_preserves_css_visibility_and_geometry() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut css = Computed::default();
        for (name, value) in [
            ("visibility", "hidden"),
            ("color", "red"),
            ("background", "blue"),
            ("border", "3px solid currentColor"),
            ("border-radius", "7px"),
            ("outline", "2px solid currentColor"),
            ("box-shadow", "1px 2px red, inset 2px 3px blue"),
        ] {
            css.apply_one(name, value, doc.values());
        }
        let original = BoxStyle::new(css);
        let hidden = original.without_own_decorations();
        assert_eq!(hidden.hidden, Some(true));
        assert_eq!(hidden.color, original.color);
        assert_eq!(hidden.background, original.background);
        assert_eq!(hidden.border_colors, original.border_colors);
        assert_eq!(hidden.borders(), original.borders());
        assert_eq!(hidden.radius, original.radius);
        assert_eq!(hidden.outline, original.outline);
        assert_eq!(hidden.shadows, original.shadows);
        assert_eq!(hidden.inset_shadows, original.inset_shadows);
        assert!(!hidden.paints_decorations());
        assert!(original.paints_decorations());
        assert!(hidden.paint_background().color.is_none());
        assert_eq!(hidden.used_border_color(0).a, 0.0);
        assert!(hidden.resolved_shadows(false).is_empty());
        assert!(hidden.resolved_shadows(true).is_empty());
        assert!(hidden.contour_outline_params().is_none());
        assert_eq!(hidden.ink(), original.ink());
    }

    #[test]
    fn background_capture_and_word_projection_keep_their_own_ink() {
        let mut css = Computed::default();
        css.color = Some(Color::BLACK);
        let white = Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        css.background = Some(white);
        let mut source = BoxStyle::new(css);
        let ink = Color {
            r: 0.5,
            ..Color::BLACK
        };
        source.own_paint_mut().foreground.color = Some(ink);
        let captured = source.without_background_color();
        assert_eq!(captured.background, source.background);
        assert!(captured.paint_background().color.is_none());
        assert_eq!(captured.ink(), Some(ink));
        let word = source.text_paint_style();
        assert_eq!(word.color, source.color);
        assert_eq!(word.foreground_paint(), source.foreground_paint());
        assert!(word.paint_background().color.is_none());
        assert_eq!(source.paint_background().color, Some(white));
    }

    #[test]
    fn physical_projection_and_pseudo_color_keep_css_and_effect_ownership() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut css = Computed::default();
        for (name, value) in [
            ("color", "red"),
            ("border-color", "red green blue black"),
            ("filter", "brightness(50%)"),
        ] {
            css.apply_one(name, value, doc.values());
        }
        let original = BoxStyle::new(css);
        let mut projected = original.clone();
        projected.project_border_paint([3, 0, 1, 2]);
        assert_eq!(projected.border_colors, original.border_colors);
        for (side, source) in [3, 0, 1, 2].into_iter().enumerate() {
            assert_eq!(
                projected.used_border_color(side),
                original.used_border_color(source)
            );
        }
        let captured = projected.without_paint_filter();
        assert_eq!(captured.filter, original.filter);
        assert!(captured.paint_filter().is_none());
        assert_eq!(captured.ink(), projected.ink());
        let blue = Color {
            b: 1.0,
            ..Color::BLACK
        };
        projected.overlay_pseudo_color(Some(blue));
        assert_eq!(projected.color, Some(blue));
        assert_eq!(projected.ink(), Some(blue));
        assert_eq!(original.color.unwrap().r, 1.0);
    }
}
