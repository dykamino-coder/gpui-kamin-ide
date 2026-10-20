//! `filter(<image>, <filter-value-list>)` over raster images.
//!
//! Filter Effects 1 §13 ("Filter CSS <image> values"): the filter list runs
//! on the input image and the result keeps the input's dimensions and origin,
//! so `drop-shadow()` pixels beyond the image are clipped. Filter regions are
//! sized by the image's concrete size; callers resolve and scale the input
//! before applying the list here.

use gpui::RenderImage;
use std::sync::Arc;

/// Applies the filter list to a straight-BGRA raster. Unsupported lists
/// (blurs, a drop shadow mixed with other functions) yield `None`.
pub(super) fn apply(
    image: &Arc<RenderImage>,
    style: &crate::style::computed::ImageFilter,
    scale: (f32, f32),
) -> Option<Arc<RenderImage>> {
    let size = image.size(0);
    let (w, h) = (size.width.0.max(0) as usize, size.height.0.max(0) as usize);
    let src = image.as_bytes(0)?;
    let mut out = src.to_vec();
    if let Some(shadow) = style.shadow {
        // Filter Effects 1 §drop-shadow: a blur-free shadow is the source
        // alpha offset by (dx, dy) in the shadow colour, under the source.
        if style.calls != 1 || shadow.blur > 0.0 {
            return None;
        }
        let (dx, dy) = (
            (shadow.x * scale.0).round() as isize,
            (shadow.y * scale.1).round() as isize,
        );
        let c = shadow.color;
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
            let c = f.apply(crate::style::values::value::Color {
                r: px[2] as f32 / 255.0,
                g: px[1] as f32 / 255.0,
                b: px[0] as f32 / 255.0,
                a: px[3] as f32 / 255.0,
            });
            let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            px.copy_from_slice(&[q(c.b), q(c.g), q(c.r), q(c.a)]);
        }
    }
    gpui::bgra_bytes_to_image(w as u32, h as u32, out)
}

#[cfg(test)]
mod tests {
    use crate::style::computed::Computed;

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
        let rendered = crate::style::cascade::inherit::inherit(&Computed::default(), &own, vcx);
        let image =
            crate::paint::background::computed_source(rendered.bg_image.as_ref().unwrap(), true)
                .unwrap()
                .raster((7.0, 5.0), vcx)
                .unwrap();
        let operand =
            crate::style::computed::ImageSource::parse(&format!("filter({raw}, invert(1))"), vcx)
                .unwrap();
        let expected = crate::paint::background::computed_source(&operand, true)
            .unwrap()
            .raster((7.0, 5.0), vcx)
            .unwrap();
        assert_eq!(image.as_bytes(0), expected.as_bytes(0));
        assert!(rendered.gradient.is_none());
    }
}
