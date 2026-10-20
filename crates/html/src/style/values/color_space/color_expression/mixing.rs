//! Color-mix operands, percentages and interpolation method retained as numbers.

use super::{Color, ColorExpression, mix};
use crate::document::state::ValueCx;
use crate::style::computed::GradSpace;

#[derive(Clone, Debug, PartialEq)]
pub struct ColorMix {
    pub(super) colors: [ColorExpression; 2],
    weights: (f32, f32),
    space: GradSpace,
    hue: u8,
}

impl ColorMix {
    pub(super) fn parse(body: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let mut it = crate::style::css::split_args(body).into_iter();
        let head = it.next()?.trim().to_ascii_lowercase();
        let method = head.strip_prefix("in ").unwrap_or("");
        let space = match method.split_whitespace().next() {
            Some(
                "srgb-linear"
                | "xyz"
                | "xyz-d50"
                | "xyz-d65"
                | "display-p3-linear"
                | "rec2020-linear"
                | "a98-rgb-linear"
                | "prophoto-rgb-linear",
            ) => GradSpace::Linear,
            Some("oklab") => GradSpace::Oklab,
            Some("oklch") => GradSpace::Oklch,
            Some("lab") => GradSpace::Lab,
            Some("lch") => GradSpace::Lch,
            Some("hsl") => GradSpace::Hsl,
            Some("hwb") => GradSpace::Hwb,
            _ => GradSpace::Srgb,
        };
        let hue = if method.contains("longer") {
            1
        } else if method.contains("increasing") {
            2
        } else if method.contains("decreasing") {
            3
        } else {
            0
        };
        let one = |raw: &str| -> Option<(ColorExpression, Option<f32>)> {
            let raw = raw.trim();
            let (color, share) = match raw.rfind('%') {
                Some(at) => {
                    let head = raw[..at].trim_end();
                    let cut = head
                        .rfind(char::is_whitespace)
                        .map(|i| i + head[i..].chars().next().map_or(1, char::len_utf8))
                        .unwrap_or(0);
                    let pct: f32 = head[cut..].parse().ok()?;
                    (raw[..cut].trim(), Some(pct / 100.0))
                }
                None => (raw, None),
            };
            let color = ColorExpression::parse(color, vcx)?;
            // Relative functions are not accepted as color-mix operands yet.
            if matches!(color, ColorExpression::Relative(_)) {
                return None;
            }
            Some((color, share))
        };
        let (a, p1) = one(it.next()?)?;
        let (b, p2) = one(it.next()?)?;
        let weights = match (p1, p2) {
            (Some(a), Some(b)) if a + b > 0.0 => (a / (a + b), b / (a + b)),
            (Some(a), None) => (a, 1.0 - a),
            (None, Some(b)) => (1.0 - b, b),
            _ => (0.5, 0.5),
        };
        Some(Self {
            colors: [a, b],
            weights,
            space,
            hue,
        })
    }

    pub(super) fn resolve(&self, current: Color, mapped: bool) -> Color {
        // Preserve the byte rounding of currentColor operands in color-mix.
        let current = Color {
            r: (current.r * 255.0).round() / 255.0,
            g: (current.g * 255.0).round() / 255.0,
            b: (current.b * 255.0).round() / 255.0,
            a: current.a,
        };
        let [a, b] = self
            .colors
            .each_ref()
            .map(|c| c.resolve_mapped(current, false));
        let (w1, w2) = self.weights;
        let alpha = a.a * w1 + b.a * w2;
        let k = if alpha > 0.0 { w2 * b.a / alpha } else { w2 };
        let (r, g, b) = mix::mix_in_impl(self.space, self.hue, a, b, k, mapped);
        Color { r, g, b, a: alpha }
    }
}
