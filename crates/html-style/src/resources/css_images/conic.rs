//! Растр `conic-gradient()` и `repeating-conic-gradient()` (CSS Images 4 §conic-gradients):
//! начальный угол `from`, центр `at`, стопы и подсказки перехода в долях оборота; лучи в
//! CSS-координатах с выборкой в центрах пикселей устройства, чтобы не интерполировать жёсткие
//! стопы второй раз на дробном DPI. Цвет в точке считает общая для градиентов `colour_at`.

use super::{len_px, wrap_repeat};
use crate::cache::{BoundedCache, FloatKey, Hinted, MIB, hint};
use crate::resources::images::types::CpuRaster;
use crate::values::ValueCx;
use std::sync::Arc;

type RasterKey = Hinted<(super::raster_keys::ImageKey, [FloatKey; 3], u32, u32)>;
// Неизменный CPU-растр сохраняет также идентичность загрузки в GPU.
static RASTERS: BoundedCache<RasterKey, Option<Arc<CpuRaster>>> =
    BoundedCache::new(64 * MIB, crate::resources::images::pixels::optional_bytes)
        .with_key_weight(|key| key.key.0.bytes());

/// CSS Images 4 §conic-gradients: positions translate the ray origin;
/// aspect ratio does not distort angles. Sampling in device pixels avoids
/// a second interpolation across hard color stops at fractional DPI.
pub fn rasterize(
    g: &crate::style::computed::ConicGradient,
    size: (f32, f32),
    density: f32,
    vcx: ValueCx<'_>,
) -> Option<Arc<CpuRaster>> {
    let image = super::raster_keys::ImageKey::conic(g, size, vcx)?;
    let (w, h) = dimensions(size, density);
    let operands = [size.0, size.1, density].map(FloatKey::from);
    let key = Hinted {
        hint: hint((operands, w, h)),
        key: (image, operands, w, h),
    };
    RASTERS.get_or_insert_with(key, || rasterize_uncached(g, size, density, vcx))
}

fn dimensions(size: (f32, f32), density: f32) -> (u32, u32) {
    (
        (size.0 * density).ceil().clamp(1.0, 4096.0) as u32,
        (size.1 * density).ceil().clamp(1.0, 4096.0) as u32,
    )
}

fn rasterize_uncached(
    g: &crate::style::computed::ConicGradient,
    size: (f32, f32),
    density: f32,
    vcx: ValueCx<'_>,
) -> Option<Arc<CpuRaster>> {
    let repeating = g.repeating;
    let from = g.from;
    let center = (
        len_px(g.position.x, size.0, vcx)?,
        len_px(g.position.y, size.1, vcx)?,
    );
    let stops = &g.stops;
    let hints = &g.hints;
    let ramp = super::colour_ramp::ramp(stops, g.space, g.hue, g.polar.as_ref(), false);
    let hard_stops: Vec<f32> = stops
        .windows(2)
        .filter(|pair| pair[0].1 == pair[1].1)
        .map(|pair| pair[0].1)
        .collect();
    let (w, h) = dimensions(size, density);
    crate::work::add!(GradientRasters, 1);
    crate::work::add!(GradientRasterPixels, u64::from(w) * u64::from(h));
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
            let color = ramp.colour_at(t);
            bytes.extend_from_slice(&[
                (color.b * 255.0).round() as u8,
                (color.g * 255.0).round() as u8,
                (color.r * 255.0).round() as u8,
                (color.a * 255.0).round() as u8,
            ]);
        }
    }
    crate::resources::images::pixels::bgra(w, h, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::ImageSource;

    #[test]
    fn conic_reuses_raster_across_documents_without_losing_sampling_operands() {
        let docs = [0, 1].map(|_| crate::values::DocumentValues::detached(Default::default()));
        let mut images = Vec::new();
        for doc in &docs {
            let ImageSource::Conic(g) = ImageSource::parse(
                "repeating-conic-gradient(from 13deg at 20% 60%, red, blue 10deg, red 30deg)",
                doc.values(),
            )
            .unwrap() else {
                panic!("conic")
            };
            let image = rasterize(&g, (18.25, 14.5), 1.25, doc.values()).unwrap();
            let direct = rasterize_uncached(&g, (18.25, 14.5), 1.25, doc.values()).unwrap();
            assert_eq!(image.bytes, direct.bytes);
            let changed = rasterize(&g, (18.5, 14.5), 1.25, doc.values()).unwrap();
            assert!(!Arc::ptr_eq(&image, &changed));
            images.push(image);
        }
        assert!(Arc::ptr_eq(&images[0], &images[1]));
    }
}
