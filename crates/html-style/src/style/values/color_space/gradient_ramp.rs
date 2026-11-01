//! Точный кусочный ramp: преобразования стопов и постоянные участки считаются
//! один раз. Непрерывные переходы не заменяются приближённой текстурной LUT.

use super::{DirectGradient, HslRamp, gradient_colour_at, prepared_mix::PreparedMix};
use crate::style::{computed::GradSpace, values::value::Color};

struct Segment {
    mix: PreparedMix,
    solid: Option<(f32, f32, f32)>,
}

pub struct GradientRamp {
    stops: Vec<(Color, f32)>,
    ends: [Color; 2],
    segments: Vec<Segment>,
    palette: Option<DirectGradient>,
    hsl: Option<HslRamp>,
    hue: u8,
}

impl GradientRamp {
    pub fn new(
        stops: &[(Color, f32)],
        space: GradSpace,
        hue: u8,
        palette: Option<&DirectGradient>,
        use_hsl: bool,
    ) -> Self {
        let ends = [stops.first(), stops.last()].map(|stop| {
            gradient_colour_at(
                stop.map(std::slice::from_ref).unwrap_or(&[]),
                f32::NAN,
                space,
                hue,
            )
        });
        let segments = stops
            .windows(2)
            .map(|p| {
                let mix = PreparedMix::new(space, hue, p[0].0, p[1].0);
                let c = p[0].0;
                let constant = [c.r, c.g, c.b, c.a].into_iter().all(f32::is_finite)
                    && c == p[1].0
                    && matches!(
                        space,
                        GradSpace::Srgb | GradSpace::Linear | GradSpace::Lab | GradSpace::Oklab
                    );
                let solid = constant.then(|| mix.at(0.0, true));
                Segment { mix, solid }
            })
            .collect();
        Self {
            stops: stops.to_vec(),
            ends,
            segments,
            palette: palette.cloned(),
            hsl: use_hsl.then(|| HslRamp::new(stops, space, hue)).flatten(),
            hue,
        }
    }

    pub fn colour_at(&self, t: f32) -> Color {
        if let Some(c) = self.hsl.as_ref().and_then(|r| r.colour_at(t)) {
            return c;
        }
        if let Some(p) = &self.palette {
            return p.colour_at(&self.stops, t, self.hue);
        }
        if self.stops.first().is_none_or(|s| t <= s.1) {
            return self.ends[0];
        }
        for (pair, segment) in self.stops.windows(2).zip(&self.segments) {
            let (a, b) = (pair[0], pair[1]);
            if t >= a.1 && t <= b.1 {
                let k = if b.1 > a.1 {
                    (t - a.1) / (b.1 - a.1)
                } else {
                    1.0
                };
                let alpha = a.0.a + (b.0.a - a.0.a) * k;
                let kc = if alpha > 0.0 { k * b.0.a / alpha } else { k };
                let (r, g, b) = segment
                    .solid
                    .filter(|_| kc.is_finite())
                    .unwrap_or_else(|| segment.mix.at(kc, true));
                return Color { r, g, b, a: alpha };
            }
        }
        self.ends[1]
    }

    pub fn bytes(&self) -> usize {
        self.hsl.as_ref().map_or(0, HslRamp::bytes)
            + std::mem::size_of::<Self>()
            + self.stops.len() * std::mem::size_of::<(Color, f32)>()
            + self.segments.len() * std::mem::size_of::<Segment>()
            + self
                .palette
                .as_ref()
                .map_or(0, |p| std::mem::size_of_val(p.operands().0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_ramp_keeps_float_results_at_jumps_alpha_and_extended_colours() {
        let stops = [
            (
                Color {
                    r: 1.3,
                    g: -0.1,
                    b: 0.2,
                    a: 0.2,
                },
                -0.2,
            ),
            (
                Color {
                    r: 0.2,
                    g: 0.4,
                    b: 0.9,
                    a: 0.8,
                },
                0.3,
            ),
            (Color::BLACK, 0.3),
            (Color::BLACK, 0.7),
            (
                Color {
                    r: 0.8,
                    g: 0.1,
                    b: 0.2,
                    a: 0.0,
                },
                1.0,
            ),
        ];
        for space in [
            GradSpace::Srgb,
            GradSpace::Linear,
            GradSpace::Lab,
            GradSpace::Lch,
            GradSpace::Oklab,
            GradSpace::Oklch,
            GradSpace::Hsl,
            GradSpace::Hwb,
        ] {
            for hue in 0..4 {
                let ramp = GradientRamp::new(&stops, space, hue, None, false);
                for t in (-100..=1100).map(|n| n as f32 / 1000.0).chain([
                    0.3f32.next_down(),
                    0.3,
                    0.3f32.next_up(),
                    f32::NAN,
                ]) {
                    assert_eq!(
                        ramp.colour_at(t),
                        super::super::gradient_stop::colour_at_with(&stops, t, |a, b, k| {
                            super::super::mix::reference_mix_in(space, hue, a, b, k, true)
                        }),
                        "space={space:?}, hue={hue}, t={t}"
                    );
                }
            }
        }
    }

    #[test]
    fn nonfinite_stop_positions_keep_endpoint_and_continuous_arithmetic() {
        for positions in [
            [f32::NEG_INFINITY, f32::INFINITY],
            [0.0, f32::INFINITY],
            [f32::NAN, 1.0],
        ] {
            let stops = positions.map(|at| (Color::BLACK, at));
            let ramp = GradientRamp::new(&stops, GradSpace::Srgb, 0, None, false);
            for t in [f32::NEG_INFINITY, 0.0, 0.5, f32::INFINITY, f32::NAN] {
                let a = ramp.colour_at(t);
                let b = gradient_colour_at(&stops, t, GradSpace::Srgb, 0);
                for (a, b) in [a.r, a.g, a.b, a.a].into_iter().zip([b.r, b.g, b.b, b.a]) {
                    assert!(a == b || (a.is_nan() && b.is_nan()), "{positions:?}, t={t}");
                }
            }
        }
    }
}
