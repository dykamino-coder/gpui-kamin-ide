//! Вычисленные операнды цвета, хранимые до момента, когда известен currentColor элемента.

use super::{interpolation_color, mix};
use crate::document::state::ValueCx;
use crate::style::values::value::Color;

mod mixing;
mod relative;
use mixing::ColorMix;
use relative::RelativeColor;

#[derive(Clone, Debug, PartialEq)]
pub enum ColorExpression {
    Color(Color),
    Current,
    Relative(RelativeColor),
    Mix(Box<ColorMix>),
}

impl ColorExpression {
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
            return RelativeColor::parse(text, vcx).map(Self::Relative);
        }
        interpolation_color(text, vcx).map(Self::Color)
    }

    pub(crate) fn depends_on_current(&self) -> bool {
        match self {
            Self::Color(_) => false,
            Self::Current => true,
            Self::Relative(relative) => relative.depends_on_current(),
            Self::Mix(mix) => mix.colors.iter().any(Self::depends_on_current),
        }
    }

    pub(crate) fn resolve(&self, current: Color) -> Color {
        self.resolve_mapped(current, true)
    }

    pub(crate) fn resolve_for_interpolation(&self, current: Color) -> Color {
        self.resolve_mapped(current, self.depends_on_current())
    }

    fn resolve_mapped(&self, current: Color, mapped: bool) -> Color {
        match self {
            Self::Color(color) => *color,
            Self::Current => current,
            Self::Relative(relative) => relative.resolve(current),
            Self::Mix(mix) => mix.resolve(current, mapped),
        }
    }
}

pub(super) fn absolute_mix(
    body: &str,
    mapped: bool,
    vcx: ValueCx<'_>,
) -> Option<(f32, f32, f32, f32)> {
    let mix = ColorMix::parse(body, vcx)?;
    if mix.colors.iter().any(ColorExpression::depends_on_current) {
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
mod tests {
    use super::*;

    #[test]
    fn relative_identity_retains_precision_and_resolves_per_element() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let a = Color {
            r: 0.1234567,
            g: 0.7654321,
            b: 0.3333333,
            a: 0.42,
        };
        let b = Color {
            r: 0.9876543,
            g: 0.1357924,
            b: 0.2468135,
            a: 0.75,
        };
        for raw in [
            "currentcolor",
            "rgb(from currentcolor r g b)",
            "hsl(from currentcolor h s l)",
            "oklch(from currentcolor l c h)",
            "color(from currentcolor xyz x y z)",
        ] {
            let expression = ColorExpression::parse(raw, vcx).unwrap();
            assert_eq!(expression.resolve(a), a);
            assert_eq!(expression.resolve(b), b);
        }
    }

    #[test]
    fn relative_channels_and_alpha_are_numeric_operands() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let c = Color {
            r: 0.2,
            g: 0.4,
            b: 0.6,
            a: 0.75,
        };
        let expression =
            ColorExpression::parse("rgb(from currentcolor b 50% r / 25%)", vcx).unwrap();
        assert_eq!(
            expression.resolve(c),
            Color {
                r: 0.6,
                g: 0.5,
                b: 0.2,
                a: 0.25
            }
        );
        assert_eq!(
            ColorExpression::parse("rgb(from currentcolor r g b / alpha)", vcx)
                .unwrap()
                .resolve(c),
            c
        );
        assert!(ColorExpression::parse("color(from currentcolor)", vcx).is_none());
    }

    #[test]
    fn current_color_mix_retains_rounding_and_premultiplied_weights() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let expression =
            ColorExpression::parse("color-mix(in srgb, currentcolor 25%, blue 75%)", vcx).unwrap();
        let c = expression.resolve(Color {
            r: 0.501,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        assert!((c.r - 128.0 / 255.0 * 0.25).abs() < f32::EPSILON);
        assert_eq!((c.g, c.b, c.a), (0.0, 0.75, 1.0));
        let c = ColorExpression::parse("color-mix(in srgb, currentcolor, blue)", vcx)
            .unwrap()
            .resolve(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            });
        assert_eq!(
            c,
            Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 0.5
            }
        );
    }

    #[test]
    fn background_expression_inherits_with_the_child_color() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        use crate::style::computed::Computed;
        let mut parent = Computed::default();
        parent.apply_one("color", "red", vcx);
        parent.apply_one(
            "background-color",
            "rgb(from currentcolor b g r / 50%)",
            vcx,
        );
        let parent = crate::style::cascade::inherit::inherit(&Computed::default(), &parent, vcx);
        assert_eq!(
            parent.background,
            Some(Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 0.5
            })
        );
        let mut child = Computed::default();
        child.apply_one("color", "blue", vcx);
        child.apply_one("background-color", "inherit", vcx);
        let child = crate::style::cascade::inherit::inherit(&parent, &child, vcx);
        assert_eq!(
            child.background,
            Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 0.5
            })
        );
    }
    #[test]
    fn absolute_mix_keeps_unmapped_interpolation_channels() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let raw = "color-mix(in oklch longer hue, red, blue)";
        let expected = interpolation_color(raw, vcx).unwrap();
        assert_eq!(
            ColorExpression::parse(raw, vcx)
                .unwrap()
                .resolve_for_interpolation(Color::default()),
            expected
        );
    }
}
