//! Пересэмплирование картинки с прозрачностью под разрешение устройства: билинейная выборка в
//! премультиплицированной альфе, чтобы цвет прозрачных точек не просачивался на край; полностью
//! непрозрачные картинки и совпадающие размеры отдаются без пересчёта, результат кэшируется.

use crate::paint::cache::{BoundedCache, MIB, optional_image_bytes};
use gpui::RenderImage;
use std::sync::Arc;

/// Бюджет 128 МиБ: самый крупный результат (4096 x 4096 точек, 64 МиБ) помещается.
static CACHE: BoundedCache<(usize, u32, u32), Option<Arc<RenderImage>>> =
    BoundedCache::new(128 * MIB, optional_image_bytes);

pub(crate) fn resample(image: &Arc<RenderImage>, w: u32, h: u32) -> Option<Arc<RenderImage>> {
    let size = image.size(0);
    let (iw, ih) = (size.width.0 as usize, size.height.0 as usize);
    if w == 0 || h == 0 || w > 4096 || h > 4096 || (iw, ih) == (w as usize, h as usize) {
        return None;
    }
    CACHE.get_or_insert_with((image.id.0, w, h), || premultiplied(image, (iw, ih), w, h))
}

/// `None` — картинка целиком непрозрачна: пересчёт не нужен.
fn premultiplied(
    image: &RenderImage,
    (iw, ih): (usize, usize),
    w: u32,
    h: u32,
) -> Option<Arc<RenderImage>> {
    let bytes = image.as_bytes(0)?;
    if bytes.chunks_exact(4).all(|p| p[3] == 255) {
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
    }
}
