//! Keep authored polar CIE/OK coordinates until interpolation and final gamut mapping.
//! CSS Color 4 section 12.2: converting a color already in the interpolation
//! space through RGB is unnecessary and can change a hue at the 0/360 seam.

use super::*;
use crate::document::state::ValueCx;
use crate::style::computed::GradSpace;
use crate::style::values::value::Color;

pub(crate) struct DirectGradient {
    colors: Vec<[f32; 3]>,
    ok: bool,
    single: bool,
}

impl DirectGradient {
    pub(crate) fn new(raw: &str, count: usize, space: GradSpace, vcx: ValueCx<'_>) -> Option<Self> {
        let (name, ok) = match space {
            GradSpace::Lch => ("lch(", false),
            GradSpace::Oklch => ("oklch(", true),
            _ => return None,
        };
        let body = &raw[raw.find('(')? + 1..raw.rfind(')')?];
        let candidates: Vec<_> = crate::style::css::split_args(body)
            .into_iter()
            .map(crate::style::computed::split_outside_parens)
            .filter(|words| {
                words
                    .first()
                    .is_some_and(|v| interpolation_color(v, vcx).is_some())
            })
            .collect();
        let single_stop = candidates.len() == 1 && candidates[0].len() == 1;
        let mut colors = Vec::new();
        for words in candidates {
            let token = words.first()?;
            let lowered = token.to_ascii_lowercase();
            let body = lowered.strip_prefix(name)?.strip_suffix(')')?;
            let (channels, _) = parts(body);
            if channels.len() != 3 || (!single_stop && channels.iter().any(|v| v == "none")) {
                return None;
            }
            let max = if ok { 1.0 } else { 100.0 };
            let value = [
                number(&channels[0], max)?.clamp(0.0, max),
                number(&channels[1], if ok { 0.4 } else { 150.0 })?,
                number(&channels[2], 1.0)?.rem_euclid(360.0),
            ];
            if value[1] <= 0.0 {
                return None;
            }
            colors.extend(std::iter::repeat_n(
                value,
                words.len().saturating_sub(1).max(1),
            ));
        }
        let single = colors.len() == 1;
        if colors.len() == 1 && count == 2 {
            colors.push(colors[0]);
        }
        (colors.len() == count).then_some(Self { colors, ok, single })
    }

    pub(crate) fn colour_at(&self, stops: &[(Color, f32)], t: f32, hue: u8) -> Color {
        let i = stops
            .windows(2)
            .position(|s| t < s[1].1)
            .unwrap_or(stops.len() - 2);
        let (a, b) = (stops[i], stops[i + 1]);
        let k = if b.1 > a.1 {
            ((t - a.1) / (b.1 - a.1)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let alpha = a.0.a + (b.0.a - a.0.a) * k;
        let kc = if alpha > 0.0 { k * b.0.a / alpha } else { k };
        let (a, b) = (self.colors[i], self.colors[i + 1]);
        let l = a[0] + (b[0] - a[0]) * kc;
        let c = a[1] + (b[1] - a[1]) * kc;
        // Hue is not premultiplied (CSS Color 4 section 12.3).
        let h = if self.single {
            a[2]
        } else {
            mix::hue_arc(a[2], b[2], hue, k)
        }
        .to_radians();
        let rgb = if self.ok {
            oklab_to_srgb(l, c * h.cos(), c * h.sin())
        } else {
            lab_to_srgb(l, c * h.cos(), c * h.sin())
        };
        let (r, g, b) = gamut_map_tuple(rgb);
        Color { r, g, b, a: alpha }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_authored_hues_produce_identical_pixels() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let render = |start| {
            let raw = format!(
                "linear-gradient(in lch decreasing hue, lch(50% 100% {start}deg), lch(50% 100% 270deg))"
            );
            let g = crate::style::computed::parse_gradient(&raw, vcx).unwrap();
            DirectGradient::new(&raw, g.stops.len(), g.space, vcx)
                .unwrap()
                .colour_at(&g.stops, 0.5, g.hue)
        };
        assert_eq!(render(0), render(360));
    }

    #[test]
    fn polar_stop_preserves_the_authored_zero_hue() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let raw =
            "linear-gradient(in lch decreasing hue, lch(50% 100% 360deg), lch(50% 100% 270deg))";
        let palette = DirectGradient::new(raw, 2, GradSpace::Lch, vcx).unwrap();
        assert_eq!(
            palette.colors,
            vec![[50.0, 150.0, 0.0], [50.0, 150.0, 270.0]]
        );
    }
}
