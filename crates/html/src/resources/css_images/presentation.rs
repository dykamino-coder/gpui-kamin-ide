//! Совместимость представления прежних CSS-картинок: codec RGBA → straight BGRA.
//! Исторический upload трактовал прямые байты кодека как premultiplied (деление с
//! усечением), затем ICC ещё раз снимал альфу и возвращал цвет с умножением. Этот
//! порядок закреплён golden-тестом; исправление цветового поведения — отдельная задача.

use crate::resources::images::{
    identity::RasterIdentity,
    pixels,
    types::{ColourSpace, CpuRaster},
};
use crate::{
    cache::{BoundedCache, MIB},
    color::{IccProfile, clamp_near_gamut},
};
use std::sync::Arc;
static CACHE: BoundedCache<RasterIdentity, Arc<CpuRaster>> =
    BoundedCache::new(128 * MIB, |r: &Arc<CpuRaster>| r.bytes.len())
        .with_key_weight(|key: &RasterIdentity| key.0.bytes.len());
pub(super) fn raster(input: &Arc<CpuRaster>) -> Arc<CpuRaster> {
    CACHE.get_or_insert_with(RasterIdentity(input.clone()), || {
        let mut bytes = pixels::straight_bgra(input);
        for p in bytes.chunks_exact_mut(4) {
            if p[3] > 0 {
                let a = p[3] as f32 / 255.0;
                for c in &mut p[..3] {
                    *c = (*c as f32 / a) as u8;
                }
            }
        }
        if let ColourSpace::Embedded(profile) = &input.colour_space
            && let Some(profile) = IccProfile::parse(profile)
        {
            for p in bytes.chunks_exact_mut(4) {
                let a = p[3] as f32 / 255.0;
                let un = |v: u8| {
                    if a > 0.0 {
                        (v as f32 / 255.0 / a).min(1.0)
                    } else {
                        0.0
                    }
                };
                let (r, g, b) =
                    clamp_near_gamut(profile.to_srgb_unmapped([un(p[2]), un(p[1]), un(p[0])]));
                p[..3].copy_from_slice(&[
                    (b * a * 255.0).round() as u8,
                    (g * a * 255.0).round() as u8,
                    (r * a * 255.0).round() as u8,
                ]);
            }
        }
        pixels::bgra(input.width, input.height, bytes).expect("dense decoded raster")
    })
}
