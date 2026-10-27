//! `filter(<image>, <filter-value-list>)` над растровыми картинками: применение уже
//! разобранного списка фильтров к straight-BGRA растру.
//!
//! Filter Effects 1 §13 («Filter CSS <image> values»): список фильтров выполняется над входной
//! картинкой, результат сохраняет её размеры и начало, поэтому точки `drop-shadow()` за
//! пределами картинки обрезаются. Область фильтра задаётся конкретным размером картинки:
//! вызывающий решает и масштабирует вход до применения списка здесь. Размытие и тень,
//! смешанная с другими функциями, не поддерживаются и дают `None`.

use crate::resources::images::types::CpuRaster;
use std::sync::Arc;

/// Applies the filter list to a straight-BGRA raster. Unsupported lists
/// (blurs, a drop shadow mixed with other functions) yield `None`.
pub(crate) fn apply(
    image: &Arc<CpuRaster>,
    style: &crate::style::computed::ImageFilter,
    scale: (f32, f32),
) -> Option<Arc<CpuRaster>> {
    let size = image;
    let (w, h) = (size.width as usize, size.height as usize);
    let src = &image.bytes;
    let mut out = src.to_vec();
    if let Some(shadow) = &style.shadow {
        // Filter Effects 1 §drop-shadow: a blur-free shadow is the source
        // alpha offset by (dx, dy) in the shadow colour, under the source.
        if style.calls != 1 || shadow.blur > 0.0 {
            return None;
        }
        let (dx, dy) = (
            (shadow.x * scale.0).round() as isize,
            (shadow.y * scale.1).round() as isize,
        );
        let c = style.shadow_used;
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (x as isize - dx, y as isize - dy);
                let sa = if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                    src[(sy as usize * w + sx as usize) * 4 + 3] as f32 / 255.0
                } else {
                    0.0
                };
                let at = (y * w + x) * 4;
                let fa = src[at + 3] as f32 / 255.0;
                let ba = sa * c.a;
                let oa = fa + ba * (1.0 - fa);
                if oa <= 0.0 {
                    out[at..at + 4].copy_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                let mix = |fc: u8, bc: f32| {
                    ((fc as f32 / 255.0 * fa + bc * ba * (1.0 - fa)) / oa * 255.0).round() as u8
                };
                out[at] = mix(src[at], c.b);
                out[at + 1] = mix(src[at + 1], c.g);
                out[at + 2] = mix(src[at + 2], c.r);
                out[at + 3] = (oa * 255.0).round() as u8;
            }
        }
    } else {
        let f = style.filter?;
        if f.blur > 0.0 {
            return None;
        }
        for px in out.chunks_exact_mut(4) {
            let c = f.apply_operations(
                &style.operations,
                crate::style::values::value::Color {
                    r: px[2] as f32 / 255.0,
                    g: px[1] as f32 / 255.0,
                    b: px[0] as f32 / 255.0,
                    a: px[3] as f32 / 255.0,
                },
            );
            let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            px.copy_from_slice(&[q(c.b), q(c.g), q(c.r), q(c.a)]);
        }
    }
    crate::resources::images::pixels::bgra(w as u32, h as u32, out)
}

#[cfg(test)]
mod tests {
    use crate::style::computed::{BoxStyle, Computed};

    #[test]
    fn element_filter_reaches_radial_samples() {
        compare_samples("radial-gradient(red, orange)");
    }

    #[test]
    fn element_filter_reaches_linear_samples() {
        compare_samples("linear-gradient(red, orange)");
    }

    fn compare_samples(raw: &str) {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut own = Computed::default();
        own.apply_one("background-image", raw, vcx);
        own.apply_one("filter", "invert(1)", vcx);
        let rendered = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
        let background = rendered.paint_background();
        let image =
            super::super::computed_source(background.top.image.as_ref().unwrap(), true, vcx)
                .unwrap()
                .raster((7.0, 5.0), vcx)
                .unwrap();
        let operand =
            crate::style::computed::ImageSource::parse(&format!("filter({raw}, invert(1))"), vcx)
                .unwrap();
        let expected = super::super::computed_source(&operand, true, vcx)
            .unwrap()
            .raster((7.0, 5.0), vcx)
            .unwrap();
        assert_eq!(image.bytes.as_ref(), expected.bytes.as_ref());
        assert!(background.gradient.is_none());
        assert_eq!(rendered.gradient, own.gradient);
        assert_eq!(rendered.bg_image, own.bg_image);
    }
}
