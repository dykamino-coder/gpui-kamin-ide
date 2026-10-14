//! `filter(<image>, <filter-value-list>)` over raster images.
//!
//! Filter Effects 1 §13 ("Filter CSS <image> values"): the filter list runs
//! on the input image and the result keeps the input's dimensions and origin,
//! so `drop-shadow()` pixels beyond the image are clipped. Filter regions are
//! sized by the image's concrete size; this module filters at the natural
//! raster size, which equals the concrete size for unscaled images.

use gpui::RenderImage;
use std::sync::Arc;

/// Splits `filter(url(x), list)` into the decoded url and the filter list.
pub(crate) fn parse(src: &str) -> Option<(String, String)> {
    let rest = src.trim().strip_prefix("filter(")?;
    let inner = &rest[..rest.rfind(')')?];
    let parts = crate::style::css::split_args(inner);
    let image = parts.first()?.trim();
    if !image.to_ascii_lowercase().starts_with("url(") || parts.len() < 2 {
        return None;
    }
    let url = crate::style::computed::parse_url(image)?;
    Some((url, parts[1..].join(" ")))
}

/// Applies the filter list to a straight-BGRA raster. Unsupported lists
/// (blurs, a drop shadow mixed with other functions) yield `None`.
pub(super) fn apply(image: &Arc<RenderImage>, list: &str) -> Option<Arc<RenderImage>> {
    let mut style = crate::style::computed::Computed::default();
    style.apply_one("filter", list);
    let calls = list.matches('(').count();
    let size = image.size(0);
    let (w, h) = (size.width.0.max(0) as usize, size.height.0.max(0) as usize);
    let src = image.as_bytes(0)?;
    let mut out = src.to_vec();
    if let Some(shadow) = style.drop_shadow {
        // Filter Effects 1 §drop-shadow: a blur-free shadow is the source
        // alpha offset by (dx, dy) in the shadow colour, under the source.
        if calls != 1 || shadow.blur > 0.0 {
            return None;
        }
        let (dx, dy) = (shadow.x.round() as isize, shadow.y.round() as isize);
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
