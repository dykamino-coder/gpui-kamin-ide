//! Растр `filter(<image>, …)` под конкретный размер плитки: вход (градиент или картинка)
//! сначала приводится к конкретному размеру объекта по CSS Images 3, затем к нему применяется
//! список фильтров (Filter Effects 1 §13), чтобы длины тени оставались в CSS-точках. Плавное
//! масштабирование идёт в премультиплицированной альфе, целочисленное увеличение и
//! `image-rendering: pixelated` — ближайшей точкой; результат кэшируется.

use super::Source;
use crate::document::state::ValueCx;
use crate::paint::cache::{BoundedCache, Hinted, MIB, hint, optional_image_bytes};
use gpui::RenderImage;
use std::sync::Arc;

type Key = Hinted<(usize, crate::style::computed::ImageFilter, u32, u32, bool)>;
/// Бюджет 64 МиБ: плитка не больше 2048 x 2048 точек (16 МиБ).
static CACHE: BoundedCache<Key, Option<Arc<RenderImage>>> =
    BoundedCache::new(64 * MIB, optional_image_bytes);

pub(super) fn raster(
    input: &Source,
    list: &crate::style::computed::ImageFilter,
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
        Source::Gradient { image } => super::gradient_raster::tile(image, w, h, tile, vcx),
        _ => input.sampled_raster(tile, pixelated, vcx),
    }?;
    let key = Hinted {
        hint: hint((image.id.0, w, h, pixelated)),
        key: (image.id.0, list.clone(), w, h, pixelated),
    };
    if list.cacheable()
        && let Some(hit) = CACHE.get(&key)
    {
        return hit;
    }
    let size = image.size(0);
    let (iw, ih) = (size.width.0 as u32, size.height.0 as u32);
    // CSS Images 3 image-rendering:auto leaves sampling to the UA.
    // Integral magnification can preserve whole texels and binary coverage.
    let integral =
        iw > 0 && ih > 0 && w >= iw && h >= ih && w.is_multiple_of(iw) && h.is_multiple_of(ih);
    let resized = resize(&image, w, h, pixelated || integral)?;
    let result = super::filter_image::apply(&resized, list, (w as f32 / tile.0, h as f32 / tile.1));
    if list.cacheable() {
        CACHE.insert(key, result.clone());
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
mod tests {
    //! Filter image sizing, coverage and allocation-density regressions.

    use super::*;
    #[test]
    fn concrete_size_keeps_shadow_lengths_in_css_pixels() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let original = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
        let input = Source::Raster(original);
        let filtered = Source::Filtered {
            image: Box::new(input),
            list: crate::style::computed::ImageFilter::parse("drop-shadow(5px 0 0 green)", vcx),
        };
        assert_eq!(filtered.intrinsic().w, Some(2.0));
        let result = filtered.sampled_raster((10.0, 10.0), true, vcx).unwrap();
        assert!(
            result
                .as_bytes(0)
                .unwrap()
                .chunks_exact(4)
                .all(|p| p == [0, 128, 0, 255])
        );
    }
    #[test]
    fn smooth_scaling_preserves_color_beside_transparency() {
        let input = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
        let result = resize(&input, 4, 1, false).unwrap();
        let pixels = result.as_bytes(0).unwrap();
        assert_eq!(&pixels[4..8], &[0, 128, 0, 191]);
        assert_eq!(&pixels[8..12], &[0, 128, 0, 64]);
    }

    #[test]
    fn allocation_limit_preserves_css_shadow_offset() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let original = gpui::bgra_bytes_to_image(2, 1, vec![0, 128, 0, 255, 0, 0, 0, 0]).unwrap();
        let input = Source::Raster(original);
        let result = raster(
            &input,
            &crate::style::computed::ImageFilter::parse("drop-shadow(1500px 0 0 green)", vcx),
            (3000.0, 10.0),
            false,
            vcx,
        )
        .unwrap();
        assert_eq!(result.size(0).width.0, 2048);
        assert!(
            result
                .as_bytes(0)
                .unwrap()
                .chunks_exact(4)
                .all(|p| p == [0, 128, 0, 255])
        );
    }

    #[test]
    fn radial_input_filters_rendered_samples_without_natural_dimensions() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let source = super::super::computed_source(
            &crate::style::computed::ImageSource::parse(
                "filter(radial-gradient(red, orange), invert(1))",
                vcx,
            )
            .unwrap(),
            true,
        )
        .unwrap();
        assert!(source.intrinsic().w.is_none());
        let filtered = source.raster((7.0, 5.0), vcx).unwrap();
        let original = super::super::gradient_raster::tile(
            &crate::style::computed::ImageSource::parse("radial-gradient(red, orange)", vcx)
                .unwrap(),
            7,
            5,
            (7.0, 5.0),
            vcx,
        )
        .unwrap();
        for (a, b) in filtered
            .as_bytes(0)
            .unwrap()
            .chunks_exact(4)
            .zip(original.as_bytes(0).unwrap().chunks_exact(4))
        {
            assert_eq!(a[3], b[3]);
            for c in 0..3 {
                assert_eq!(a[c], 255 - b[c]);
            }
        }
    }
}
