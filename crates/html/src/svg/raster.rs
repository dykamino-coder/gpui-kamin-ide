//! Растеризация разметки через resvg и кэш готовых растров.

use super::DENSITY;
use gpui::RenderImage;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// Кэш готовых растров: ключ — разметка и размер.
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
    let key = (hasher.finish(), w.round() as u32, h.round() as u32);

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(&key)
    {
        return hit.clone();
    }
    // Растеризатор берём у самого GPUI: держать второй комплект resvg/usvg
    // ради той же работы — лишний вес бинаря и лишний источник расхождений.
    let image = gpui::svg_markup_to_image(markup, w, h, DENSITY);
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
    let key = (hasher.finish(), w.round() as u32, h.round() as u32);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some(hit) = map.get(&key)
    {
        return hit.clone();
    }
    let image = gpui::svg_markup_to_image_padded(markup, w, h, DENSITY, pad);
    if let Ok(mut map) = cache.lock() {
        if map.len() >= CACHE_CAP {
            map.clear();
        }
        map.insert(key, image.clone());
    }
    image
}
