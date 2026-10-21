//! Растеризация разметки через resvg и кэш готовых растров.

use super::DENSITY;
use gpui::RenderImage;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// Кэш готовых растров: ключ — разметка и размер растра в пикселях устройства.
///
/// Без него рисунок растрировался бы на КАЖДЫЙ кадр — элементы в GPUI
/// пересоздаются каждый раз, и вместе с ними пересоздавалась бы картинка.
/// Для графика в чате это десятки миллисекунд на кадр вместо нуля.
type Cache = HashMap<(u64, u32, u32), Option<Arc<RenderImage>>>;

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

/// Потолок: рисунки в чате мелкие, но их может накопиться много за длинную
/// сессию — держим последние, а не всё подряд.
const CACHE_CAP: usize = 128;

/// Растрировать рисунок. `None`, если разметка не разбирается — тогда
/// вызывающий покажет запасной текст, а не пустое место.
pub fn rasterize(markup: &str, w: f32, h: f32) -> Option<Arc<RenderImage>> {
    let mut hasher = DefaultHasher::new();
    markup.hash(&mut hasher);
    cached(hasher.finish(), w, h, || {
        gpui::svg_markup_to_image(markup, w, h, DENSITY)
    })
}

/// Device pixels of one raster axis, normalised exactly as gpui's SVG
/// rasterizer sizes its pixmap (`(logical * density).round().max(1)`).
///
/// The cache key must be the raster's real size: two requests that round to
/// the same logical pixel (10.1 and 10.4) still produce 20 and 21 device
/// pixels, while two that land on the same device size produce byte-identical
/// rasters and may share one entry.
pub(super) fn device_extent(logical: f32) -> u32 {
    (logical * DENSITY).round().max(1.0) as u32
}

/// Reuse the bounded raster cache for markup and typed decoration scenes.
/// `create` must rasterize at [`DENSITY`]: the key is its device-pixel size.
pub(super) fn cached(
    hash: u64,
    w: f32,
    h: f32,
    create: impl FnOnce() -> Option<Arc<RenderImage>>,
) -> Option<Arc<RenderImage>> {
    let key = (hash, device_extent(w), device_extent(h));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(&key)
    {
        return hit.clone();
    }
    let image = create();
    if let Ok(mut map) = cache.lock() {
        if map.len() >= CACHE_CAP {
            map.clear();
        }
        map.insert(key, image.clone());
    }
    image
}

/// `rasterize` with `pad` logical pixels of extra canvas on the right and
/// bottom at the same user-space scale (`gpui::svg_markup_to_image_padded`).
pub(super) fn rasterize_padded(markup: &str, w: f32, h: f32, pad: f32) -> Option<Arc<RenderImage>> {
    let mut hasher = DefaultHasher::new();
    markup.hash(&mut hasher);
    pad.to_bits().hash(&mut hasher);
    cached(hasher.finish(), w, h, || {
        gpui::svg_markup_to_image_padded(markup, w, h, DENSITY, pad)
    })
}

#[cfg(test)]
mod tests {
    use super::{DENSITY, device_extent, rasterize};

    const MARKUP: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'>                          <rect width='10' height='10' fill='green'/><!--cache-key--></svg>";

    #[test]
    fn cache_returns_raster_of_requested_device_size() {
        // 10.1 and 10.4 round to one logical pixel but to 20 and 21 device
        // pixels; the second request must not reuse the first raster.
        let a = rasterize(MARKUP, 10.1, 10.0).expect("a");
        let b = rasterize(MARKUP, 10.4, 10.0).expect("b");
        let direct = gpui::svg_markup_to_image(MARKUP, 10.4, 10.0, DENSITY).expect("direct");
        assert_eq!(a.size(0).width.0, 20);
        assert_eq!(b.size(0).width, direct.size(0).width);
        assert_eq!(b.size(0).width.0, 21);
    }

    #[test]
    fn device_extent_matches_gpui_pixmap_size() {
        for tenth in 0..60 {
            let w = tenth as f32 / 10.0;
            let image = gpui::svg_markup_to_image(MARKUP, w, 3.0, DENSITY).expect("raster");
            assert_eq!(image.size(0).width.0 as u32, device_extent(w), "width {w}");
        }
        assert_eq!(device_extent(f32::NAN), 1);
    }
}
