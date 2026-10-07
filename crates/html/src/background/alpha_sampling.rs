//! Filter background image colors in premultiplied alpha at device resolution.

use gpui::RenderImage;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

type Cache = HashMap<(usize, u32, u32), Option<Arc<RenderImage>>>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

pub(super) fn resample(image: &Arc<RenderImage>, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    let size = image.size(0);
    let (iw, ih) = (size.width.0 as usize, size.height.0 as usize);
    if w == 0 || h == 0 || w > 4096 || h > 4096 || (iw, ih) == (w as usize, h as usize) {
        return None;
    }
    let key = (image.id.0, w, h);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(found) = cache.get(&key)
    {
        return found.clone();
    }
    let bytes = image.as_bytes(0)?;
    let result = if bytes.chunks_exact(4).all(|p| p[3] == 255) {
        None
    } else {
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let fy = (y as f32 + 0.5) * ih as f32 / h as f32 - 0.5;
            for x in 0..w {
                let fx = (x as f32 + 0.5) * iw as f32 / w as f32 - 0.5;
                let (mut color, mut alpha) = ([0.0f32; 3], 0.0f32);
                for (dy, wy) in [(0, 1.0 - (fy - fy.floor())), (1, (fy - fy.floor()))] {
                    for (dx, wx) in [(0, 1.0 - (fx - fx.floor())), (1, (fx - fx.floor()))] {
                        let sx = (fx.floor() as isize + dx).clamp(0, iw as isize - 1) as usize;
                        let sy = (fy.floor() as isize + dy).clamp(0, ih as isize - 1) as usize;
                        let p = &bytes[(sy * iw + sx) * 4..][..4];
                        let a = p[3] as f32 * wx * wy;
                        alpha += a;
                        for c in 0..3 {
                            color[c] += p[c] as f32 * a;
                        }
                    }
                }
                for c in color {
                    out.push(if alpha > 0.0 {
                        (c / alpha).round() as u8
                    } else {
                        0
                    });
                }
                out.push(alpha.round() as u8);
            }
        }
        gpui::bgra_bytes_to_image(w, h, out)
    };
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(key, result.clone());
    }
    result
}
