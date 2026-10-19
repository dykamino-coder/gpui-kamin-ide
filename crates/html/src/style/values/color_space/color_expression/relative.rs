//! Relative color channels and alpha, parsed independently of the current color.

use super::Color;
use crate::document::state::ValueCx;
use crate::style::values::color_space::{hsl_to_rgb, rgb_to_hsl};

#[derive(Clone, Debug, PartialEq)]
pub struct RelativeColor {
    base: Option<Color>,
    alpha: Option<f32>,
    channels: [Channel; 3],
    model: Model,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Channel {
    Base(usize),
    Number(f32),
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Model {
    Identity,
    Rgb,
    Hsl,
}

impl RelativeColor {
    pub(super) fn parse(expr: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let open = expr.find('(')?;
        let name = expr[..open].trim();
        let rest = expr[open + 1..]
            .trim()
            .strip_suffix(')')?
            .trim()
            .strip_prefix("from ")?;
        let words = crate::style::computed::split_outside_parens(rest);
        let base_token = words.first()?;
        let base = if base_token.eq_ignore_ascii_case("currentcolor") {
            None
        } else {
            Some(Color::parse(base_token, vcx)?)
        };
        let mut alpha = None;
        let mut channels = Vec::new();
        let mut after_slash = false;
        for word in &words[1..] {
            if word == "/" {
                after_slash = true;
            } else if after_slash {
                alpha = word
                    .strip_suffix('%')
                    .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                    .or_else(|| word.parse().ok());
            } else {
                channels.push(word.as_str());
            }
        }
        let identity: &[&str] = match name {
            "rgb" | "rgba" => &["r", "g", "b"],
            "hsl" | "hsla" => &["h", "s", "l"],
            "hwb" => &["h", "w", "b"],
            "lab" | "oklab" => &["l", "a", "b"],
            "lch" | "oklch" => &["l", "c", "h"],
            "color" => {
                let (space, rest) = channels.split_first()?;
                let names: &[&str] = if space.starts_with("xyz") {
                    &["x", "y", "z"]
                } else {
                    &["r", "g", "b"]
                };
                if rest != names {
                    return None;
                }
                return Some(Self {
                    base,
                    alpha,
                    channels: [Channel::Base(0), Channel::Base(1), Channel::Base(2)],
                    model: Model::Identity,
                });
            }
            _ => return None,
        };
        let mut result = Self {
            base,
            alpha,
            channels: [Channel::Base(0), Channel::Base(1), Channel::Base(2)],
            model: Model::Identity,
        };
        if channels == identity {
            return Some(result);
        }
        result.model = match name {
            "rgb" | "rgba" => Model::Rgb,
            "hsl" | "hsla" => Model::Hsl,
            _ => return None,
        };
        for (i, target) in result.channels.iter_mut().enumerate() {
            let token = *channels.get(i)?;
            *target = if let Some(index) = identity.iter().position(|name| *name == token) {
                Channel::Base(index)
            } else {
                Channel::Number(match result.model {
                    Model::Rgb => token
                        .strip_suffix('%')
                        .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                        .or_else(|| token.parse::<f32>().ok().map(|v| v / 255.0))?,
                    Model::Hsl if i == 0 => {
                        token.strip_suffix("deg").unwrap_or(token).parse().ok()?
                    }
                    Model::Hsl => token
                        .strip_suffix('%')
                        .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                        .or_else(|| token.parse().ok())?,
                    Model::Identity => return None,
                })
            };
        }
        Some(result)
    }

    pub(super) fn depends_on_current(&self) -> bool {
        self.base.is_none()
    }

    pub(super) fn resolve(&self, current: Color) -> Color {
        let base = self.base.unwrap_or(current);
        let alpha = self.alpha.unwrap_or(base.a);
        if self.model == Model::Identity {
            return Color { a: alpha, ..base };
        }
        let values = match self.model {
            Model::Hsl => {
                let (h, s, l) = rgb_to_hsl(base);
                [h, s, l]
            }
            _ => [base.r, base.g, base.b],
        };
        let mut channels = self.channels.map(|c| match c {
            Channel::Base(i) => values[i],
            Channel::Number(v) => v,
        });
        if self.model == Model::Hsl {
            let (r, g, b) = hsl_to_rgb(
                channels[0],
                channels[1].clamp(0.0, 1.0),
                channels[2].clamp(0.0, 1.0),
            );
            Color { r, g, b, a: alpha }
        } else {
            channels = channels.map(|v| v.clamp(0.0, 1.0));
            Color {
                r: channels[0],
                g: channels[1],
                b: channels[2],
                a: alpha,
            }
        }
    }
}
