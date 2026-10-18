//! Resolve CSS Images 3 concrete object size before applying Filter Effects 1 §13.

use super::Source;
use crate::document::state::ValueCx;
use gpui::RenderImage;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

type Key = (usize, String, u32, u32, bool);
type Cache = Mutex<HashMap<Key, Option<Arc<RenderImage>>>>;
static CACHE: OnceLock<Cache> = OnceLock::new();

pub(super) fn raster(
    input: &Source,
    list: &str,
    tile: (f32, f32),
    pixelated: bool,
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    if tile.0 <= 0.0 || tile.1 <= 0.0 {
        return None;
    }
    let (w, h) = (
        tile.0.round().clamp(1.0, 2048.0) as u32,
        tile.1.round().clamp(1.0, 2048.0) as u32,
    );
    let image = match input {
        Source::Gradient { raw } => super::gradient_raster::tile(raw, w, h, tile, vcx),
        _ => input.sampled_raster(tile, pixelated, vcx),
    }?;
    let key = (image.id.0, list.to_string(), w, h, pixelated);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let size = image.size(0);
    let (iw, ih) = (size.width.0 as u32, size.height.0 as u32);
    // CSS Images 3 image-rendering:auto leaves sampling to the UA.
    // Integral magnification can preserve whole texels and binary coverage.
    let integral =
        iw > 0 && ih > 0 && w >= iw && h >= ih && w.is_multiple_of(iw) && h.is_multiple_of(ih);
    let resized = resize(&image, w, h, pixelated || integral)?;
    let result =
        super::filter_image::apply(&resized, list, (w as f32 / tile.0, h as f32 / tile.1), vcx);
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(key, result.clone());
    }
    result
}

fn resize(image: &Arc<RenderImage>, w: u32, h: u32, nearest: bool) -> Option<Arc<RenderImage>> {
    let s = image.size(0);
    let (iw, ih) = (s.width.0 as usize, s.height.0 as usize);
    if (iw, ih) == (w as usize, h as usize) {
        return Some(image.clone());
    }
    if iw == 0 || ih == 0 {
        return None;
    }
    let bytes = image.as_bytes(0)?;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let fy = (y as f32 + 0.5) * ih as f32 / h as f32 - 0.5;
        for x in 0..w {
            let fx = (x as f32 + 0.5) * iw as f32 / w as f32 - 0.5;
            if nearest {
                let sx = (fx + 0.5).floor().clamp(0.0, iw as f32 - 1.0) as usize;
                let sy = (fy + 0.5).floor().clamp(0.0, ih as f32 - 1.0) as usize;
                out.extend_from_slice(&bytes[(sy * iw + sx) * 4..][..4]);
                continue;
            }
            // Smooth resizing operates in premultiplied alpha, then returns
            // the straight BGRA expected by the image shader/filter evaluator.
            let (mut color, mut alpha) = ([0.0f32; 3], 0.0f32);
            for (dy, wy) in [
                (0, 1.0 - fy.fract().rem_euclid(1.0)),
                (1, fy.fract().rem_euclid(1.0)),
            ] {
                for (dx, wx) in [
                    (0, 1.0 - fx.fract().rem_euclid(1.0)),
                    (1, fx.fract().rem_euclid(1.0)),
                ] {
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
}

#[cfg(test)]
mod tests;
