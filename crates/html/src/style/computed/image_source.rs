//! Вычисленные операнды изображений; декодирование ресурсов принадлежит paint.

use super::{Computed, Filter, Gradient, Len, Shadow};
use crate::document::state::ValueCx;
use crate::style::values::{color_space::StyleColor, value::Color};

mod conic;
mod filter;
mod parse;
mod resolve;
pub(crate) use conic::ConicGradient;

#[derive(Clone, Debug, PartialEq)]
pub struct ImageColor {
    expression: StyleColor,
    pub(crate) value: Color,
}
impl ImageColor {
    fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let expression = StyleColor::parse(raw, vcx)?;
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageFilter {
    pub(crate) filter: Option<Filter>,
    pub(crate) shadow: Option<Shadow>,
    pub(crate) calls: usize,
    /// Used color of `shadow` for the element painting the image
    /// (`resolve_colors`; images keep used values like `ImageSource::Color`).
    pub(crate) shadow_used: Color,
    pub(in crate::style::computed) blur_length: Option<Len>,
    pub(in crate::style::computed) shadow_value: Option<super::parse_util::ShadowValue>,
}

impl ImageFilter {
    pub(crate) fn color_filter(filter: Filter) -> Self {
        Self {
            filter: Some(Filter {
                blur: 0.0,
                ..filter
            }),
            calls: 8,
            ..Self::default()
        }
    }

    pub(crate) fn parse(raw: &str, vcx: ValueCx<'_>) -> Self {
        let mut style = Computed::default();
        style.parse_filter(raw, vcx)
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
            Self::Filter { image, program } => {
                image.resolve_colors(current);
                program.resolve_colors(current);
            }
            Self::Color(color) => color.resolve(current),
            Self::Fallback {
                color: Some(color), ..
            } => color.resolve(current),
            _ => {}
        }
    }

    /// Retain the historical element-filter stop quantization without CSS text.
    pub(crate) fn filter_stops(&mut self, filter: &Filter) {
        let apply = |color: &mut Color| {
            *color = filter.apply(*color);
            color.r = (color.r * 255.0).round() / 255.0;
            color.g = (color.g * 255.0).round() / 255.0;
            color.b = (color.b * 255.0).round() / 255.0;
        };
        match self {
            Self::Gradient {
                value,
                repeating: true,
            } => {
                apply(&mut value.from);
                apply(&mut value.to);
                for (color, _) in &mut value.stops {
                    apply(color);
                }
                for (color, _) in &mut value.stops_px {
                    apply(color);
                }
                for (color, _, _) in &mut value.stops_raw {
                    apply(color);
                }
                value.polar = None;
                value.colors.clear();
                if !value.explicit_interpolation {
                    value.space = super::GradSpace::Srgb;
                }
            }
            Self::Conic(value) => {
                for (color, _) in &mut value.stops {
                    apply(color);
                }
                value.polar = None;
                value.colors.clear();
                if !value.explicit_interpolation {
                    value.space = super::GradSpace::Srgb;
                }
            }
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

impl Computed {
    pub(crate) fn content_image(&self) -> Option<&ImageSource> {
        match self.content.as_deref() {
            Some([super::ContentItem::Image(image)]) => Some(image),
            _ => None,
        }
    }
}
