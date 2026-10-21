//! Растр линейных и радиальных градиентов и `cross-fade()`: стопы решаются по длине
//! градиентной линии (CSS Images 3 §3.2.3, §3.4.3), цвет берётся в центрах пикселей устройства
//! в выбранном интерполяционном пространстве и квантуется в straight-alpha BGRA; плитки
//! кэшируются по записи, размеру растра и конкретному CSS-размеру.

use super::{gradient_geometry::Geometry, place_stops, wrap_repeat};
use crate::document::state::ValueCx;
use crate::style::values::color_space::gradient_colour_at as colour_at;
use gpui::RenderImage;
use std::sync::{Arc, Mutex, OnceLock};

type TileKey = (crate::style::computed::ImageSource, u32, u32, u32, u32);
type TileCache = Mutex<Vec<(TileKey, Option<Arc<RenderImage>>)>>;
static TILES: OnceLock<TileCache> = OnceLock::new();

/// Device-sized samples depend on the concrete CSS line as well as the bitmap.
pub(super) fn tile(
    src: &crate::style::computed::ImageSource,
    w: u32,
    h: u32,
    css: (f32, f32),
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    if !src.cacheable() {
        return raster(src, w, h, css, true, vcx);
    }
    let key = (src.clone(), w, h, css.0.to_bits(), css.1.to_bits());
    let cache = TILES.get_or_init(Default::default);
    if let Some(hit) = cache
        .lock()
        .ok()
        .and_then(|c| c.iter().find(|(k, _)| k == &key).map(|(_, v)| v.clone()))
    {
        return hit;
    }
    let result = raster(src, w, h, css, true, vcx);
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.push((key, result.clone()));
    }
    result
}

pub(super) fn raster(
    src: &crate::style::computed::ImageSource,
    w: u32,
    h: u32,
    css: (f32, f32),
    centers: bool,
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    let Some((g, repeating)) = src.gradient() else {
        return super::raster::rasterize_gradient(src, w, h, vcx);
    };
    let angle = g.shape.corner_angle(css).unwrap_or(g.angle_deg);
    let geometry = Geometry::new(g, css, angle, vcx)?;
    // CSS Images 3 sections 3.2.3 and 3.4.3: percentages refer to the
    // gradient line, which is the horizontal radius for a radial gradient.
    let stops = resolved_stops(g, geometry.length());
    let hsl = crate::style::values::color_space::HslRamp::new(&stops, g.space, g.hue);
    let palette = &g.polar;
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
            let t = geometry.at(point, css);
            let t = if repeating { wrap_repeat(t, &stops) } else { t };
            let colour = hsl
                .as_ref()
                .and_then(|ramp| ramp.colour_at(t))
                .unwrap_or_else(|| {
                    palette.as_ref().map_or_else(
                        || colour_at(&stops, t, g.space, g.hue),
                        |palette| palette.colour_at(&stops, t, g.hue),
                    )
                });
            // The image upload shader premultiplies straight-alpha samples.
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

pub(super) fn resolved_stops(
    g: &crate::style::computed::Gradient,
    axis: f32,
) -> Vec<(crate::style::values::value::Color, f32)> {
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
    place_stops(raw)
}
