//! Conic gradient rays in CSS coordinates, sampled at device pixel centers.

use super::{Source, len_px, wrap_repeat};
use crate::document::state::ValueCx;
use crate::style::values::color_space::gradient_colour_at as colour_at;
use gpui::RenderImage;
use std::sync::Arc;

/// CSS Images 4 §conic-gradients: positions translate the ray origin;
/// aspect ratio does not distort angles. Sampling in device pixels avoids
/// a second interpolation across hard color stops at fractional DPI.
pub(super) fn rasterize(
    g: &crate::style::computed::ConicGradient,
    size: (f32, f32),
    density: f32,
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    let repeating = g.repeating;
    let from = g.from;
    let center = (
        len_px(g.position.x, size.0, vcx)?,
        len_px(g.position.y, size.1, vcx)?,
    );
    let stops = &g.stops;
    let hints = &g.hints;
    let hard_stops: Vec<f32> = stops
        .windows(2)
        .filter(|pair| pair[0].1 == pair[1].1)
        .map(|pair| pair[0].1)
        .collect();
    let (w, h) = (
        (size.0 * density).ceil().clamp(1.0, 4096.0) as u32,
        (size.1 * density).ceil().clamp(1.0, 4096.0) as u32,
    );
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5) * size.0 / w as f32 - center.0;
            let dy = (y as f32 + 0.5) * size.1 / h as f32 - center.1;
            let t = (dx.atan2(-dy) / std::f32::consts::TAU - from).rem_euclid(1.0);
            let mut t = if repeating { wrap_repeat(t, stops) } else { t };
            if !repeating {
                // CSS Images 4 sections 3.3 and 3.5.2: the finite sweep ends
                // on the starting ray, and coincident stops jump clockwise.
                // Choose the closed endpoint and the following side of a jump.
                if t == 0.0 {
                    t = 1.0;
                }
                if hard_stops.contains(&t) {
                    t = t.next_up();
                }
            }
            // CSS Images 3 §3.4.2: a transition hint places the 50% mix
            // at the hint and uses an exponential curve between the stops.
            for &(index, hint) in hints {
                if index == 0 || index >= stops.len() {
                    continue;
                }
                let (a, b) = (stops[index - 1].1, stops[index].1);
                if t > a && t < b {
                    let h = ((hint - a) / (b - a)).clamp(0.0, 1.0);
                    let mix = if h == 0.0 {
                        1.0
                    } else if h == 1.0 {
                        0.0
                    } else {
                        ((t - a) / (b - a)).powf(0.5f32.ln() / h.ln())
                    };
                    t = a + (b - a) * mix;
                }
            }
            let color = g.polar.as_ref().map_or_else(
                || colour_at(stops, t, g.space, g.hue),
                |p| p.colour_at(stops, t, g.hue),
            );
            bytes.extend_from_slice(&[
                (color.b * 255.0).round() as u8,
                (color.g * 255.0).round() as u8,
                (color.r * 255.0).round() as u8,
                (color.a * 255.0).round() as u8,
            ]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}

pub(super) fn tile(
    source: &Source,
    size: (f32, f32),
    density: f32,
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    match source {
        Source::Gradient { image } => match image.as_ref() {
            crate::style::computed::ImageSource::Conic(g) => rasterize(g, size, density, vcx),
            _ => source.raster(size, vcx),
        },
        _ => source.raster(size, vcx),
    }
}
