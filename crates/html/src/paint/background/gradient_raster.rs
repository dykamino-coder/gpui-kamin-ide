//! Gradient rasterization retains CSS stop lengths at the mask device resolution.

use crate::style::values::color_space::gradient_colour_at as colour_at;
use super::{angle_fraction, place_stops, rasterize_cross_fade, wrap_repeat};
use gpui::RenderImage;
use std::sync::Arc;

pub(super) fn raster(
    src: &str,
    w: u32,
    h: u32,
    css: (f32, f32),
    centers: bool,
) -> Option<Arc<RenderImage>> {
    if src.starts_with("cross-fade(") {
        return rasterize_cross_fade(src, w, h);
    }
    enum Mode {
        Axis { dx: f32, dy: f32 },
        Sweep { from: f32 },
    }
    // CSS Images 3 section 3.6: repeat the stop interval across the gradient line.
    let repeating = src.starts_with("repeating-");
    let src = src.strip_prefix("repeating-").unwrap_or(src);
    let (mode, stops, space, hue) = if let Some(inner) = src
        .strip_prefix("conic-gradient(")
        .and_then(|t| t.strip_suffix(')'))
    {
        let parts = crate::style::css::split_args(inner);
        let mut idx = 0usize;
        let mut from = 0.0f32;
        if let Some(first) = parts.first().map(|f| f.trim())
            && (first.starts_with("from ") || first.starts_with("at "))
        {
            idx = 1;
            if let Some(a) = first.strip_prefix("from ") {
                from = angle_fraction(a.split_whitespace().next().unwrap_or("")).unwrap_or(0.0);
            }
        }
        let mut raw: Vec<(crate::style::values::value::Color, Option<f32>)> = vec![];
        for part in &parts[idx..] {
            let words = crate::style::computed::split_outside_parens(part);
            let Some(colour) = words
                .first()
                .and_then(|w| crate::style::values::color_space::interpolation_color(w))
            else {
                continue;
            };
            let angles: Vec<f32> = words[1..]
                .iter()
                .filter_map(|w| angle_fraction(w))
                .collect();
            if angles.is_empty() {
                raw.push((colour, None));
            }
            for a in angles {
                raw.push((colour, Some(a)));
            }
        }
        if raw.is_empty() {
            return None;
        }
        (
            Mode::Sweep { from },
            place_stops(raw),
            crate::style::computed::GradSpace::Srgb,
            0u8,
        )
    } else {
        let g = crate::style::computed::parse_gradient(src)?;
        let angle = g.angle_deg.to_radians();
        let (dx, dy) = (angle.sin(), -angle.cos());
        // Absolute stop lengths use the CSS line length, independent of raster density.
        let stops = if g.stops_raw.iter().any(|(_, _, p)| p.is_some()) {
            let axis = (css.0 * dx).abs() + (css.1 * dy).abs();
            let raw: Vec<(crate::style::values::value::Color, Option<f32>)> = g
                .stops_raw
                .iter()
                .map(|(c, f, p)| {
                    let px = p.map(|v| if axis > 0.0 { v / axis } else { 0.0 });
                    let at = match (f, px) {
                        (Some(f), Some(px)) => Some(f + px),
                        (f, px) => f.or(px),
                    };
                    (*c, at)
                })
                .collect();
            place_stops(raw)
        } else {
            g.stops.clone()
        };
        (Mode::Axis { dx, dy }, stops, g.space, g.hue)
    };

    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = if centers {
                (
                    (x as f32 + 0.5) / w as f32 - 0.5,
                    (y as f32 + 0.5) / h as f32 - 0.5,
                )
            } else {
                (
                    x as f32 / (w.max(2) - 1) as f32 - 0.5,
                    y as f32 / (h.max(2) - 1) as f32 - 0.5,
                )
            };
            let t = match mode {
                Mode::Axis { dx, dy } => {
                    let t = if centers {
                        // CSS Images 3 section 3.4.1: project the pixel centre onto
                        // the gradient line, whose length includes both box axes.
                        let axis = (css.0 * dx).abs() + (css.1 * dy).abs();
                        (fx * css.0 * dx + fy * css.1 * dy) / axis.max(f32::EPSILON)
                    } else {
                        fx * dx + fy * dy
                    };
                    (t + 0.5).clamp(0.0, 1.0)
                }
                Mode::Sweep { from } => {
                    let turn = if centers {
                        (fx * css.0).atan2(-fy * css.1)
                    } else {
                        fx.atan2(-fy)
                    } / std::f32::consts::TAU;
                    (turn - from).rem_euclid(1.0)
                }
            };
            let t = if repeating { wrap_repeat(t, &stops) } else { t };
            let colour = colour_at(&stops, t, space, hue);
            // CSS Color 4 §5.1: reducing calculated component precision
            // rounds to the nearest integer (ties toward +infinity).
            // Truncation darkens a constant interpolated color relative to
            // the same color painted directly. Masks store premultiplied BGRA.
            bytes.push((colour.b * colour.a * 255.0).round() as u8);
            bytes.push((colour.g * colour.a * 255.0).round() as u8);
            bytes.push((colour.r * colour.a * 255.0).round() as u8);
            bytes.push((colour.a * 255.0) as u8);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}
