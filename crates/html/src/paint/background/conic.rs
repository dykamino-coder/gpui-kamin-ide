//! Conic gradient rays in CSS coordinates, sampled at device pixel centers.

use super::{Source, angle_fraction, len_px, place_stops, split_top, wrap_repeat};
use crate::style::computed::parse_pos_words;
use crate::style::values::color_space::gradient_colour_at as colour_at;
use gpui::RenderImage;
use std::sync::Arc;

pub(super) fn is_conic(raw: &str) -> bool {
    raw.strip_prefix("repeating-")
        .unwrap_or(raw)
        .starts_with("conic-gradient(")
}

/// CSS Images 4 §conic-gradients: positions translate the ray origin;
/// aspect ratio does not distort angles. Sampling in device pixels avoids
/// a second interpolation across hard color stops at fractional DPI.
pub(super) fn rasterize(raw: &str, size: (f32, f32), density: f32) -> Option<Arc<RenderImage>> {
    let repeating = raw.starts_with("repeating-");
    let raw = raw.strip_prefix("repeating-").unwrap_or(raw);
    let inner = raw.strip_prefix("conic-gradient(")?.strip_suffix(')')?;
    let parts = crate::style::css::split_args(inner);
    let mut idx = 0;
    let mut from = 0.0;
    let mut center = (size.0 * 0.5, size.1 * 0.5);
    let mut interpolation = String::new();
    if let Some(head) = parts.first() {
        let words = split_top(head.trim());
        if words
            .first()
            .is_some_and(|w| matches!(*w, "from" | "at" | "in"))
        {
            idx = 1;
            let mut i = 0;
            while i < words.len() {
                match words[i] {
                    "from" => {
                        // Preserve the existing fallback for angle calculations not yet parsed.
                        from = angle_fraction(words.get(i + 1)?).unwrap_or(0.0);
                        i += 2;
                    }
                    "at" | "in" => {
                        let kind = words[i];
                        let start = i + 1;
                        i = start;
                        while i < words.len() && !matches!(words[i], "from" | "at" | "in") {
                            i += 1;
                        }
                        let value = words[start..i].join(" ");
                        if kind == "at" {
                            let position = parse_pos_words(&value);
                            center = (len_px(position.x, size.0)?, len_px(position.y, size.1)?);
                        } else {
                            interpolation = format!("in {value}, ");
                        }
                    }
                    _ => return None,
                }
            }
        }
    }
    let mut colors = Vec::new();
    let mut raw_stops = Vec::new();
    for part in &parts[idx..] {
        let words = crate::style::computed::split_outside_parens(part);
        let Some(color) = words
            .first()
            .and_then(|w| crate::style::values::color_space::interpolation_color(w))
        else {
            continue;
        };
        colors.push(words[0].clone());
        let positions: Vec<f32> = words[1..]
            .iter()
            .filter_map(|w| angle_fraction(w))
            .collect();
        if positions.is_empty() {
            raw_stops.push((color, None));
        } else {
            raw_stops.extend(positions.into_iter().map(|p| (color, Some(p))));
        }
    }
    if raw_stops.is_empty() {
        return None;
    }
    // Reuse interpolation-space selection and hue rules for all gradient kinds.
    let method = crate::style::computed::parse_gradient(&format!(
        "linear-gradient({interpolation}{})",
        colors.join(", ")
    ))?;
    let stops = place_stops(raw_stops);
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
            let mut t = if repeating { wrap_repeat(t, &stops) } else { t };
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
            let color = colour_at(&stops, t, method.space, method.hue);
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

pub(super) fn tile(source: &Source, size: (f32, f32), density: f32) -> Option<Arc<RenderImage>> {
    match source {
        Source::Gradient { raw } if is_conic(raw) => rasterize(raw, size, density),
        _ => source.raster(size),
    }
}
