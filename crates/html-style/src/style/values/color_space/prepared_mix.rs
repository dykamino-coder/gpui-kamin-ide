//! Неизменные координаты концов цветового перехода (CSS Color 4 §12).
//! Порядок операций выборки совпадает с `mix_in`; преобразования стопов общие.

use super::*;
use crate::style::{computed::GradSpace, values::value::Color};

pub(super) struct PreparedMix {
    space: GradSpace,
    hue: u8,
    a: [f32; 3],
    b: [f32; 3],
}

impl PreparedMix {
    pub(super) fn new(space: GradSpace, hue: u8, a: Color, b: Color) -> Self {
        let convert = |c: Color| match space {
            GradSpace::Srgb => (c.r, c.g, c.b),
            GradSpace::Linear => (srgb_linear(c.r), srgb_linear(c.g), srgb_linear(c.b)),
            GradSpace::Oklab | GradSpace::Oklch => srgb_to_oklab(c),
            GradSpace::Lab | GradSpace::Lch => srgb_to_lab(c),
            GradSpace::Hsl | GradSpace::Hwb => rgb_to_hsl(c),
        };
        let (al, ax, ay) = convert(a);
        let (bl, bx, by) = convert(b);
        let axes = |l: f32, x: f32, y: f32| {
            if matches!(space, GradSpace::Oklch | GradSpace::Lch) {
                [l, (x * x + y * y).sqrt(), y.atan2(x).to_degrees()]
            } else {
                [l, x, y]
            }
        };
        Self {
            space,
            hue,
            a: axes(al, ax, ay),
            b: axes(bl, bx, by),
        }
    }

    pub(super) fn at(&self, k: f32, mapped: bool) -> (f32, f32, f32) {
        let [a, x, y] = self.a;
        let [b, u, v] = self.b;
        let lerp = |a: f32, b: f32| a + (b - a) * k;
        let polar = || {
            let l = lerp(a, b);
            let c = lerp(x, u);
            let h = mix::hue_arc(y, v, self.hue, k).to_radians();
            (l, c * h.cos(), c * h.sin())
        };
        let map = |r, g, b| {
            if mapped {
                gamut_map(r, g, b)
            } else {
                (r, g, b)
            }
        };
        let map_tuple = |v| if mapped { gamut_map_tuple(v) } else { v };
        match self.space {
            GradSpace::Srgb => map(lerp(a, b), lerp(x, u), lerp(y, v)),
            GradSpace::Linear => map(
                srgb_gamma(lerp(a, b)),
                srgb_gamma(lerp(x, u)),
                srgb_gamma(lerp(y, v)),
            ),
            GradSpace::Oklab | GradSpace::Oklch => {
                let (l, x, y) = if self.space == GradSpace::Oklch {
                    polar()
                } else {
                    (lerp(a, b), lerp(x, u), lerp(y, v))
                };
                map_tuple(oklab_to_srgb(l, x, y))
            }
            GradSpace::Lab | GradSpace::Lch => {
                let (l, x, y) = if self.space == GradSpace::Lch {
                    polar()
                } else {
                    (lerp(a, b), lerp(x, u), lerp(y, v))
                };
                map_tuple(lab_to_srgb(l, x, y))
            }
            GradSpace::Hsl | GradSpace::Hwb => {
                hsl_to_rgb(mix::hue_arc(a, b, self.hue, k), lerp(x, u), lerp(y, v))
            }
        }
    }
}
