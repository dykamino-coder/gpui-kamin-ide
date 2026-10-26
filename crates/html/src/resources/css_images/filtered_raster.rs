//! Растр `filter(<image>, …)` под конкретный размер плитки: вход (градиент или картинка)
//! сначала приводится к конкретному размеру объекта по CSS Images 3, затем к нему применяется
//! список фильтров (Filter Effects 1 §13), чтобы длины тени оставались в CSS-точках. Плавное
//! масштабирование идёт в премультиплицированной альфе, целочисленное увеличение и
//! `image-rendering: pixelated` — ближайшей точкой; результат кэшируется.

use super::Source;
use crate::cache::{BoundedCache, FloatKey, Hinted, MIB, hint};
use crate::document::state::ValueCx;
use crate::resources::images::pixels::optional_bytes as optional_image_bytes;
use crate::resources::images::types::CpuRaster;
use std::sync::Arc;

use crate::resources::images::identity::RasterIdentity;
type Key = Hinted<(
    RasterIdentity,
    super::raster_keys::FilterKey,
    u32,
    u32,
    bool,
    [FloatKey; 2],
)>;
/// Бюджет 64 МиБ: плитка не больше 2048 x 2048 точек (16 МиБ).
static CACHE: BoundedCache<Key, Option<Arc<CpuRaster>>> =
    BoundedCache::new(64 * MIB, optional_image_bytes)
        .with_key_weight(|k: &Key| k.key.0.0.bytes.len());

pub(super) fn raster(
    input: &Source,
    list: &crate::style::computed::ImageFilter,
    tile: (f32, f32),
    pixelated: bool,
    vcx: ValueCx<'_>,
) -> Option<Arc<CpuRaster>> {
    if tile.0 <= 0.0 || tile.1 <= 0.0 {
        return None;
    }
    let (w, h) = (
        tile.0.round().clamp(1.0, 2048.0) as u32,
        tile.1.round().clamp(1.0, 2048.0) as u32,
    );
    let image = match input {
        Source::Gradient { image } => super::gradient_raster::tile(image, w, h, tile, vcx),
        _ => input.sampled_raster(tile, pixelated, vcx),
    }?;
    let scale = (w as f32 / tile.0, h as f32 / tile.1);
    let key = Hinted {
        hint: hint((std::sync::Arc::as_ptr(&image) as usize, w, h, pixelated)),
        key: (
            RasterIdentity(image.clone()),
            list.into(),
            w,
            h,
            pixelated,
            [scale.0, scale.1].map(FloatKey::from),
        ),
    };
    if list.cacheable()
        && let Some(hit) = CACHE.get(&key)
    {
        return hit;
    }
    let (iw, ih) = (image.width, image.height);
    // CSS Images 3 image-rendering:auto leaves sampling to the UA.
    // Integral magnification can preserve whole texels and binary coverage.
    let integral =
        iw > 0 && ih > 0 && w >= iw && h >= ih && w.is_multiple_of(iw) && h.is_multiple_of(ih);
    let resized = resize(&image, w, h, pixelated || integral)?;
    let result = super::filter_image::apply(&resized, list, scale);
    if list.cacheable() {
        CACHE.insert(key, result.clone());
    }
    result
}

fn resize(image: &Arc<CpuRaster>, w: u32, h: u32, nearest: bool) -> Option<Arc<CpuRaster>> {
    let s = image;
    let (iw, ih) = (s.width as usize, s.height as usize);
    if (iw, ih) == (w as usize, h as usize) {
        return Some(image.clone());
    }
    if iw == 0 || ih == 0 {
        return None;
    }
    use crate::resources::images::sampling::{Sampling, resize};
    let sampling = if nearest {
        Sampling::Nearest
    } else {
        Sampling::Bilinear(|v| v.fract().rem_euclid(1.0))
    };
    let out = resize(&image.bytes, (iw, ih), (w, h), sampling);
    crate::resources::images::pixels::bgra(w, h, out)
}

#[cfg(test)]
mod tests;
