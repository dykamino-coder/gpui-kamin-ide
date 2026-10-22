//! Пересэмплирование картинки с прозрачностью под разрешение устройства: билинейная выборка в
//! премультиплицированной альфе, чтобы цвет прозрачных точек не просачивался на край; полностью
//! непрозрачные картинки и совпадающие размеры отдаются без пересчёта, результат кэшируется.

use super::{
    identity::RasterIdentity,
    pixels::{bgra, optional_bytes, straight_bgra},
    types::CpuRaster,
};
use crate::cache::{BoundedCache, MIB};
use std::sync::Arc;

/// Бюджет 128 МиБ: самый крупный результат (4096 x 4096 точек, 64 МиБ) помещается.
static CACHE: BoundedCache<(RasterIdentity, u32, u32), Option<Arc<CpuRaster>>> =
    BoundedCache::new(128 * MIB, optional_bytes)
        .with_key_weight(|key: &(RasterIdentity, u32, u32)| key.0.0.bytes.len());

pub(crate) fn resample(image: &Arc<CpuRaster>, w: u32, h: u32) -> Option<Arc<CpuRaster>> {
    let (iw, ih) = (image.width as usize, image.height as usize);
    if w == 0 || h == 0 || w > 4096 || h > 4096 || (iw, ih) == (w as usize, h as usize) {
        return None;
    }
    CACHE.get_or_insert_with((RasterIdentity(image.clone()), w, h), || {
        premultiplied(image, (iw, ih), w, h)
    })
}

/// `None` — картинка целиком непрозрачна: пересчёт не нужен.
fn premultiplied(
    image: &CpuRaster,
    (iw, ih): (usize, usize),
    w: u32,
    h: u32,
) -> Option<Arc<CpuRaster>> {
    let bytes = straight_bgra(image);
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
        bgra(w, h, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_neighbour_does_not_tint_the_edge() {
        let input = bgra(2, 1, vec![0, 128, 0, 255, 255, 0, 0, 0]).unwrap();
        let result = resample(&input, 4, 1).unwrap();
        assert_eq!(
            result.bytes.as_ref(),
            &[0, 128, 0, 255, 0, 128, 0, 191, 0, 128, 0, 64, 0, 0, 0, 0]
        );
        assert!(Arc::ptr_eq(&result, &resample(&input, 4, 1).unwrap()));
    }

    #[test]
    fn opaque_and_unchanged_images_keep_original_sampling() {
        let image = bgra(1, 1, vec![3, 5, 7, 255]).unwrap();
        assert!(resample(&image, 2, 2).is_none());
        assert!(resample(&image, 1, 1).is_none());
        assert!(resample(&image, 0, 2).is_none());
        assert!(resample(&image, 4097, 1).is_none());
    }
}
