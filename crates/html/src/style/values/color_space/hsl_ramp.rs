//! Цветовые пространства: градиент `in hsl` как набор линейных отрезков в
//! глобальной координате стопа — коэффициенты `factor·t + bias` считаются один
//! раз на отрезок (как Skia `SkGradientBaseShader::init_stop_pos`), поэтому
//! вставка коллинеарного стопа не копит ошибку округления локального `k`.
//! Дуга тона — по `hue` (shorter/longer/increasing/decreasing); переменная
//! прозрачность уходит на общий премультиплицированный путь.

use super::{hsl_to_rgb, rgb_to_hsl};
use crate::style::{computed::GradSpace, values::value::Color};

pub(crate) struct HslRamp(Vec<Segment>);

struct Segment {
    start: f32,
    end: f32,
    factor: [f32; 3],
    bias: [f32; 3],
    alpha: f32,
}

impl HslRamp {
    pub(crate) fn new(stops: &[(Color, f32)], space: GradSpace, hue: u8) -> Option<Self> {
        if !matches!(space, GradSpace::Hsl) {
            return None;
        }
        let mut segments = Vec::new();
        for pair in stops.windows(2) {
            let ((a, start), (b, end)) = (pair[0], pair[1]);
            // Keep the general premultiplied-alpha path for variable opacity.
            if a.a != b.a || end <= start {
                return None;
            }
            let (ah, as_, al) = rgb_to_hsl(a);
            let (bh, bs, bl) = rgb_to_hsl(b);
            let mut delta = (bh - ah).rem_euclid(360.0);
            match hue {
                1 => {
                    if delta > 180.0 {
                        delta -= 360.0;
                    }
                    if delta > 0.0 {
                        delta -= 360.0;
                    } else {
                        delta += 360.0;
                    }
                }
                2 => {}
                3 => {
                    if delta > 0.0 {
                        delta -= 360.0;
                    }
                }
                _ => {
                    if delta > 180.0 {
                        delta -= 360.0;
                    }
                }
            }
            let left = [ah, as_, al];
            let right = [ah + delta, bs, bl];
            // Skia SkGradientBaseShader::init_stop_pos computes reciprocal
            // once, then uses factor*t+bias. Subdividing a linear ramp keeps
            // the same coefficients instead of accumulating local-k rounding.
            let reciprocal = 1.0 / (end - start);
            if !reciprocal.is_finite() {
                return None;
            }
            let factor = std::array::from_fn(|i| (right[i] - left[i]) * reciprocal);
            let bias = std::array::from_fn(|i| left[i] - factor[i] * start);
            segments.push(Segment {
                start,
                end,
                factor,
                bias,
                alpha: a.a,
            });
        }
        Some(Self(segments))
    }

    pub(crate) fn colour_at(&self, t: f32) -> Option<Color> {
        let segment = self.0.iter().find(|s| t > s.start && t < s.end)?;
        let axes: [f32; 3] = std::array::from_fn(|i| segment.factor[i] * t + segment.bias[i]);
        let (r, g, b) = hsl_to_rgb(axes[0], axes[1], axes[2]);
        Some(Color {
            r,
            g,
            b,
            a: segment.alpha,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserting_a_collinear_stop_preserves_quantized_samples() {
        let red = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let green = Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };
        let blue = Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        };
        let whole = HslRamp::new(&[(red, 0.2), (blue, 0.8)], GradSpace::Hsl, 1).unwrap();
        let split =
            HslRamp::new(&[(red, 0.2), (green, 0.5), (blue, 0.8)], GradSpace::Hsl, 0).unwrap();
        let quantize = |c: Color| [c.r, c.g, c.b].map(|v| (v * 255.0).round() as u8);
        for x in 20..80 {
            let t = (x as f32 + 0.5) / 100.0;
            assert_eq!(
                quantize(whole.colour_at(t).unwrap()),
                quantize(split.colour_at(t).unwrap())
            );
        }
    }
}
