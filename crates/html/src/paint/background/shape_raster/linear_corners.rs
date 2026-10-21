//! Маски `rrect` с прямолинейными углами (`corner-shape: bevel`-плоские и `notch`): контур
//! строится многоугольником и идёт тем же путём покрытия, что обрезка `clip-path: polygon()`
//! — до восьми вершин бинарным покрытием на сетке устройства, иначе через SVG и свёртку маски.
//! CSS Borders 4 §corner-shape-rendering смещает сам контур: вогнутая выемка растёт во
//! вдвинутый прямоугольник вместе с внутренними краями. Кольцо рамки — дополнение внутренней
//! маски цветом рамки в straight BGRA.

use super::{CORNER_K_FLAT, Rrect};
use crate::document::state::ValueCx;
use crate::paint::background::{MaskLayer, compose_mask_layers};
use crate::style::values::value::Color;
use gpui::RenderImage;
use std::sync::Arc;

fn corner(rx: f32, ry: f32, k: f32, bx: f32, by: f32) -> Option<Vec<(f32, f32)>> {
    if rx <= 0.0 || ry <= 0.0 || k >= CORNER_K_FLAT {
        return Some(vec![(bx, by)]);
    }
    if k <= -CORNER_K_FLAT {
        return Some(vec![(rx + bx, by), (rx + bx, ry + by), (bx, ry + by)]);
    }
    None
}

pub(super) fn contour(
    r: &Rrect,
    w: f32,
    h: f32,
    [t, right, b, l]: [f32; 4],
) -> Option<Vec<(f32, f32)>> {
    // Curved and bevel corners keep their existing coverage backend.
    if r.corners
        .iter()
        .zip(r.k)
        .any(|(&(rx, ry), k)| rx > 0.0 && ry > 0.0 && k > -CORNER_K_FLAT && k < CORNER_K_FLAT)
    {
        return None;
    }
    if l + right >= w || t + b >= h {
        return Some(Vec::new());
    }
    let transforms = [
        (1, w, 0.0, -1.0, 1.0, right, t, false),
        (2, w, h, -1.0, -1.0, right, b, true),
        (3, 0.0, h, 1.0, -1.0, l, b, false),
        (0, 0.0, 0.0, 1.0, 1.0, l, t, true),
    ];
    let mut points = Vec::new();
    for (i, ox, oy, sx, sy, bx, by, reverse) in transforms {
        let (rx, ry) = r.corners[i];
        let mut own = corner(rx, ry, r.k[i], bx, by)?;
        if reverse {
            own.reverse();
        }
        points.extend(own.into_iter().map(|(x, y)| (ox + sx * x, oy + sy * y)));
    }
    // The existing raster path handles intersecting corner cut-outs. A
    // simple polygon is only equivalent while successive side edges keep
    // their clockwise order; do not turn overlapping notches into a loop.
    for i in 0..points.len() {
        let a = points[i];
        let c = points[(i + 1) % points.len()];
        if a.1 == c.1 && ((a.1 == t && a.0 > c.0) || (a.1 == h - b && a.0 < c.0))
            || a.0 == c.0 && ((a.0 == w - right && a.1 > c.1) || (a.0 == l && a.1 < c.1))
        {
            return None;
        }
    }
    Some(points)
}

pub(super) fn mask(
    r: &Rrect,
    w: u32,
    h: u32,
    scale: f32,
    inset: [f32; 4],
    origin: (f32, f32),
) -> Option<Arc<RenderImage>> {
    let (fw, fh) = (w as f32, h as f32);
    let points = contour(r, fw, fh, inset)?;
    // Match polygon_clip's rectilinear edge grid in absolute device space.
    // Return to CSS coordinates only after snapping, preserving the same
    // origin subtraction used by the path-based clip compositor.
    let points: Vec<_> = points
        .into_iter()
        .map(|(x, y)| {
            (
                ((x + origin.0 * scale).round() / scale - origin.0),
                ((y + origin.1 * scale).round() / scale - origin.1),
            )
        })
        .collect();
    if points.len() <= 8 {
        return native_mask(&points, w, h, scale);
    }
    let mut d = String::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        d.push_str(&format!("{}{x} {y} ", if i == 0 { 'M' } else { 'L' }));
    }
    d.push('Z');
    let (cw, ch) = (fw / scale, fh / scale);
    let markup = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><path fill="#ffffff" d="{d}"/></svg>"##
    );
    let image = crate::svg::raster::rasterize(&markup, cw, ch)?;
    // Keep source coverage through composition and sample once on the device
    // grid, as a clip-path polygon with the same contour does.
    compose_mask_layers(
        &[MaskLayer {
            image,
            tile: [0.0, 0.0, fw, fh],
            no_repeat: (true, true),
            gap: (0.0, 0.0),
            snap: false,
            op: 0,
            luminance: true,
        }],
        w,
        h,
    )
}

fn native_mask(points: &[(f32, f32)], w: u32, h: u32, scale: f32) -> Option<Arc<RenderImage>> {
    // Up to eight polygon vertices use the native shader. Axis-aligned
    // snapped edges have binary coverage at device pixel centres.
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            let mut inside = false;
            for (&(ax, ay), &(bx, by)) in points.iter().zip(points.iter().cycle().skip(1)) {
                if (ay > py) != (by > py) && px < ax + (bx - ax) * (py - ay) / (by - ay) {
                    inside = !inside;
                }
            }
            bytes.extend_from_slice(&[if inside { 255 } else { 0 }; 4]);
        }
    }
    gpui::bgra_bytes_to_image(w, h, bytes)
}

pub fn rasterize_shape_at(
    raw: &str,
    w: u32,
    h: u32,
    scale: f32,
    origin: (f32, f32),
    vcx: ValueCx<'_>,
) -> Option<Arc<RenderImage>> {
    if let Some(args) = raw.strip_prefix("rrect(").and_then(|s| s.strip_suffix(')')) {
        let r = super::parse_rrect(args, w as f32, h as f32, scale)?;
        if r.ring.is_none()
            && let Some(image) = mask(&r, w, h, scale, [0.0; 4], origin)
        {
            return Some(image);
        }
    }
    super::rasterize_shape(raw, w, h, scale, vcx)
}

pub fn rasterize_ring_at(
    args: &str,
    w: u32,
    h: u32,
    scale: f32,
    colour: Color,
    origin: (f32, f32),
) -> Option<Arc<RenderImage>> {
    let r = super::parse_rrect(args, w as f32, h as f32, scale)?;
    if let Some(image) = mask(&r, w, h, scale, r.ring?, origin) {
        return ring(&image, colour);
    }
    super::rasterize_ring(args, w, h, scale, colour)
}

pub(super) fn ring(mask: &Arc<RenderImage>, colour: Color) -> Option<Arc<RenderImage>> {
    let bytes = mask.as_bytes(0)?;
    let size = mask.size(0);
    // The outer group clips the completed border box. Inside it, overlaying
    // the complement of inner coverage is equivalent to painting the outer
    // border colour first and then the inner background. Images use straight
    // BGRA; storing premultiplied RGB here darkens antialiased border edges.
    let mut out = Vec::with_capacity(bytes.len());
    for pixel in bytes.chunks_exact(4) {
        out.extend_from_slice(&[
            (colour.b * 255.0).round() as u8,
            (colour.g * 255.0).round() as u8,
            (colour.r * 255.0).round() as u8,
            ((255 - pixel[3]) as f32 * colour.a).round() as u8,
        ]);
    }
    gpui::bgra_bytes_to_image(size.width.0 as u32, size.height.0 as u32, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_notch_retains_the_border_along_its_concave_edges() {
        let r = Rrect {
            corners: [(31.25, 31.25); 4],
            k: [-CORNER_K_FLAT; 4],
            ring: None,
        };
        let outer = mask(&r, 125, 125, 1.25, [0.0; 4], (0.0, 0.0)).unwrap();
        let inner = mask(&r, 125, 125, 1.25, [6.25; 4], (0.0, 0.0)).unwrap();
        let alpha = |image: &Arc<RenderImage>, x: usize, y: usize| {
            image.as_bytes(0).unwrap()[(y * 125 + x) * 4 + 3]
        };
        // The 5px inset extends the 25px notch to 30px. This point lies
        // outside the original corner rectangle but still inside the border.
        assert_eq!(alpha(&outer, 33, 33), 255);
        assert_eq!(alpha(&inner, 33, 33), 0);
        assert_eq!(alpha(&inner, 62, 62), 255);
    }
}
