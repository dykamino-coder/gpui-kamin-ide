//! Sample gradient geometry and stop lengths before quantizing straight-alpha BGRA.

use super::{gradient_geometry::Geometry, place_stops, rasterize_cross_fade, wrap_repeat};
use crate::style::values::color_space::gradient_colour_at as colour_at;
use gpui::RenderImage;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

type TileKey = (String, u32, u32, u32, u32);
type TileCache = Mutex<HashMap<TileKey, Option<Arc<RenderImage>>>>;
static TILES: OnceLock<TileCache> = OnceLock::new();

/// Device-sized samples depend on the concrete CSS line as well as the bitmap.
pub(super) fn tile(src: &str, w: u32, h: u32, css: (f32, f32)) -> Option<Arc<RenderImage>> {
    let key = (src.to_string(), w, h, css.0.to_bits(), css.1.to_bits());
    let cache = TILES.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let result = raster(src, w, h, css, true);
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(key, result.clone());
    }
    result
}

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
    let repeating = src.starts_with("repeating-");
    let g = crate::style::computed::parse_gradient(src)?;
    let angle = crate::style::computed::corner_angle(src, css).unwrap_or(g.angle_deg);
    let geometry = Geometry::new(src, css, angle)?;
    // CSS Images 3 sections 3.2.3 and 3.4.3: percentages refer to the
    // gradient line, which is the horizontal radius for a radial gradient.
    let axis = geometry.length();
    let raw = g
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
    let stops = place_stops(raw);
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let point = if centers {
                (
                    (x as f32 + 0.5) * css.0 / w as f32,
                    (y as f32 + 0.5) * css.1 / h as f32,
                )
            } else {
                (
                    x as f32 * css.0 / (w.max(2) - 1) as f32,
                    y as f32 * css.1 / (h.max(2) - 1) as f32,
                )
            };
            // Repeating gradients extend along the entire line, including
            // outside the 0..100% interval. Clamp only the final color lookup.
            let t = geometry.at(point, css);
            let t = if repeating { wrap_repeat(t, &stops) } else { t };
            let colour = colour_at(&stops, t, g.space, g.hue);
            // The image upload shader premultiplies straight-alpha samples.
            // Premultiplying here as well darkens transparent gradients twice.
            bytes.extend_from_slice(&[
                (colour.b * 255.0).round() as u8,
                (colour.g * 255.0).round() as u8,
                (colour.r * 255.0).round() as u8,
                (colour.a * 255.0).round() as u8,
            ]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}
