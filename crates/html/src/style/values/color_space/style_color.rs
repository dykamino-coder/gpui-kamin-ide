//! Вычисленное значение `<color>` (css-color-4 §4.6, §6.4; css-color-5 §2,
//! §4.1): абсолютный цвет, слово `currentcolor` или функция (`color-mix()`,
//! относительный цвет), содержащая его. Слово остаётся словом и в
//! вычисленном, и в унаследованном значении; цветом оно становится только
//! при использовании — от вычисленного `color` того элемента, где
//! используется (`StyleColor::resolve`). Тот же механизм у Gecko
//! (`GenericColor::{Absolute, CurrentColor, ColorMix}`) и Blink (`StyleColor`
//! с `UnresolvedColorFunction`).

use super::{interpolation_color, mix};
use crate::document::state::ValueCx;
use crate::style::values::value::Color;

mod mixing;
mod relative;
use mixing::ColorMix;
use relative::RelativeColor;

#[derive(Clone, Debug, PartialEq)]
pub enum StyleColor {
    Absolute(Color),
    Current,
    Relative(Box<RelativeColor>),
    Mix(Box<ColorMix>),
}

/// Начальное значение большинства цветовых свойств — `currentcolor`
/// (`border-*-color`, `column-rule-color`, `text-decoration-color`,
/// `text-emphasis-color`, …).
impl Default for StyleColor {
    fn default() -> Self {
        Self::Current
    }
}

impl From<Color> for StyleColor {
    fn from(color: Color) -> Self {
        Self::Absolute(color)
    }
}

impl StyleColor {
    /// Значение свойства: абсолютные цвета — цвета вывода (`Color::parse`,
    /// с отображением в охват sRGB), `light-dark()` — ветвь используемой
    /// схемы (css-color-5 §light-dark), в том числе `currentcolor`.
    pub(crate) fn parse_value(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let text = raw.trim();
        if text.eq_ignore_ascii_case("currentcolor") {
            return Some(Self::Current);
        }
        if let Some(branch) = crate::style::values::value::light_dark_branch(text, vcx) {
            return Self::parse_value(branch?, vcx);
        }
        let lower = text.to_ascii_lowercase();
        if lower.contains("currentcolor") || lower.contains("(from ") {
            return Self::parse(text, vcx).map(Self::settle);
        }
        Color::parse(text, vcx).map(Self::Absolute)
    }

    /// Операнд интерполяции (градиент, `color-mix()`): абсолютный цвет
    /// без отображения в охват — отображается только итог.
    pub(crate) fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let text = raw.trim();
        if text.eq_ignore_ascii_case("currentcolor") {
            return Some(Self::Current);
        }
        let lower = text.to_ascii_lowercase();
        if lower.starts_with("color-mix(") && text.ends_with(')') {
            return ColorMix::parse(&lower[10..lower.len() - 1], vcx)
                .map(|m| Self::Mix(Box::new(m)));
        }
        if text.contains("(from ") {
            return RelativeColor::parse(text, vcx).map(|r| Self::Relative(Box::new(r)));
        }
        interpolation_color(text, vcx).map(Self::Absolute)
    }

    /// Функция без `currentcolor` вычисляется в абсолютный цвет сразу.
    fn settle(self) -> Self {
        if self.depends_on_current() {
            self
        } else {
            Self::Absolute(self.resolve(Color::default()))
        }
    }

    pub(crate) fn is_current(&self) -> bool {
        matches!(self, Self::Current)
    }

    pub(crate) fn depends_on_current(&self) -> bool {
        match self {
            Self::Absolute(_) => false,
            Self::Current => true,
            Self::Relative(relative) => relative.depends_on_current(),
            Self::Mix(mix) => mix.colors.iter().any(Self::depends_on_current),
        }
    }

    /// Используемый цвет: `currentcolor` — вычисленный `color` элемента.
    pub(crate) fn resolve(&self, current: Color) -> Color {
        self.resolve_mapped(current, true)
    }

    pub(crate) fn resolve_for_interpolation(&self, current: Color) -> Color {
        self.resolve_mapped(current, self.depends_on_current())
    }

    fn resolve_mapped(&self, current: Color, mapped: bool) -> Color {
        match self {
            Self::Absolute(color) => *color,
            Self::Current => current,
            Self::Relative(relative) => relative.resolve(current),
            Self::Mix(mix) => mix.resolve(current, mapped),
        }
    }
}

/// `auto | <color>` (css-ui-4 `caret-color`, `accent-color`, `outline-color`):
/// `auto` — значение, а не отсутствие объявления.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ColorOrAuto {
    #[default]
    Auto,
    Color(StyleColor),
}

impl ColorOrAuto {
    pub(crate) fn parse_value(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        if raw.trim().eq_ignore_ascii_case("auto") {
            return Some(Self::Auto);
        }
        StyleColor::parse_value(raw, vcx).map(Self::Color)
    }

    pub(crate) fn color(&self) -> Option<&StyleColor> {
        match self {
            Self::Auto => None,
            Self::Color(color) => Some(color),
        }
    }
}

pub(super) fn absolute_mix(
    body: &str,
    mapped: bool,
    vcx: ValueCx<'_>,
) -> Option<(f32, f32, f32, f32)> {
    let mix = ColorMix::parse(body, vcx)?;
    if mix.colors.iter().any(StyleColor::depends_on_current) {
        return None;
    }
    let color = mix.resolve(
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        mapped,
    );
    Some((color.r, color.g, color.b, color.a))
}

#[cfg(test)]
mod tests;
