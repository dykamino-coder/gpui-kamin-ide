//! Computed image operands; resource decoding belongs to paint.

use super::{Computed, Filter, Gradient, Len, Shadow};
use crate::document::state::ValueCx;
use crate::style::values::{color_space::ColorExpression, value::Color};

mod conic;
mod parse;
mod resolve;
pub(crate) use conic::ConicGradient;

#[derive(Clone, Debug, PartialEq)]
pub struct ImageColor {
    expression: ColorExpression,
    pub(crate) value: Color,
}
impl ImageColor {
    fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let expression = ColorExpression::parse(raw, vcx)?;
        let value = expression.resolve(Color {
            a: 1.0,
            ..Color::default()
        });
        Some(Self { expression, value })
    }
    fn resolve(&mut self, current: Color) {
        self.value = self.expression.resolve(current);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImageSource {
    Url(String),
    Gradient {
        value: Box<Gradient>,
        repeating: bool,
    },
    Conic(Box<ConicGradient>),
    Color(ImageColor),
    Fallback {
        url: String,
        color: Option<ImageColor>,
    },
    CrossFade(Vec<(ImageSource, f32)>),
    Filter {
        image: Box<ImageSource>,
        program: ImageFilter,
    },
    Invalid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageFilter {
    pub(crate) filter: Option<Filter>,
    pub(crate) shadow: Option<Shadow>,
    pub(crate) calls: usize,
}

impl ImageFilter {
    pub(crate) fn parse(raw: &str, vcx: ValueCx<'_>) -> Self {
        let mut style = Computed::default();
        style.apply_one("filter", raw, vcx);
        Self {
            filter: style.filter,
            shadow: style.drop_shadow,
            calls: raw.matches('(').count(),
        }
    }
}

impl ImageSource {
    pub(crate) fn resolve_colors(&mut self, current: Color) {
        match self {
            Self::Gradient { value, .. } => value.resolve_colors(current),
            Self::Conic(value) => value.resolve_colors(current),
            Self::CrossFade(items) => {
                for (image, _) in items {
                    image.resolve_colors(current);
                }
            }
            Self::Filter { image, .. } => image.resolve_colors(current),
            Self::Color(color) => color.resolve(current),
            Self::Fallback {
                color: Some(color), ..
            } => color.resolve(current),
            _ => {}
        }
    }

    pub(crate) fn gradient(&self) -> Option<(&Gradient, bool)> {
        match self {
            Self::Gradient { value, repeating } => Some((value, *repeating)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
