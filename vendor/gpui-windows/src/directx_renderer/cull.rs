//! KaminIDE patch: the instances of a frame that write inside its redraw
//! rectangles (`Plan::Within`, `Plan::Shift`), gathered for one pass.
//!
//! A pass per rectangle over every batch costs a draw call per batch and
//! rectangle; on WARP a draw call costs more than the instances it draws,
//! so splitting batches into more runs made scroll steps slower. So the rectangles are made disjoint and every instance that can
//! reach one is copied once per rectangle it reaches, with its content mask
//! cut to that rectangle: one upload, one draw per run of batches of one kind
//! that reach any rectangle, and the masks keep every pixel outside the
//! rectangles as the scissor did. A batch that reaches none is left out, so
//! the batches of one kind around it join into one draw. WebRender batches
//! the primitives of its dirty tiles the same way (`batch.rs`, per-target
//! instance lists over the dirty rect), Chromium raster records only the
//! display items of the invalidated rect (`DisplayItemList::Raster` with an
//! `rtree` query).
//!
//! The cut mask is exact: an edge the rectangle does not move keeps its
//! value (the shader computes `origin + size` to the same float), and an
//! edge it moves lies on a whole pixel, half a pixel from every pixel centre.

use super::group_region::Region;
use gpui::{Bounds, ContentMask, ScaledPixels, TransformationMatrix};

mod pass;
#[cfg(test)]
mod tests;

/// The device pixels an instance can write: its bounds grown by `margin`
/// (antialiasing, a shadow's blur), within its mask; `None` when a
/// transformation moves it elsewhere (it reaches every rectangle).
pub(super) fn extent(
    bounds: Bounds<ScaledPixels>,
    mask: &ContentMask<ScaledPixels>,
    transformation: Option<&TransformationMatrix>,
    margin: f32,
) -> Option<[f32; 4]> {
    if transformation.is_some_and(|t| *t != TransformationMatrix::unit()) {
        return None;
    }
    let (o, s) = (bounds.origin, bounds.size);
    let (mo, ms) = (mask.bounds.origin, mask.bounds.size);
    Some([
        (o.x.0 - margin).max(mo.x.0),
        (o.y.0 - margin).max(mo.y.0),
        (o.x.0 + s.width.0 + margin).min(mo.x.0 + ms.width.0),
        (o.y.0 + s.height.0 + margin).min(mo.y.0 + ms.height.0),
    ])
}

/// Whether an instance of extent `e` can write inside `r` (x0, y0, x1, y1).
pub(super) fn meets(e: Option<[f32; 4]>, r: &[f32; 4]) -> bool {
    e.is_none_or(|e| e[0] < r[2] && e[2] > r[0] && e[1] < r[3] && e[3] > r[1])
}

/// `mask` cut to `r` (x0, y0, x1, y1); `false` when nothing is left.
pub(super) fn cut(mask: &mut ContentMask<ScaledPixels>, r: &[f32; 4]) -> bool {
    let (o, s) = (&mut mask.bounds.origin, &mut mask.bounds.size);
    for (origin, size, lo, hi) in [
        (&mut o.x, &mut s.width, r[0], r[2]),
        (&mut o.y, &mut s.height, r[1], r[3]),
    ] {
        let end = origin.0 + size.0;
        let (start, stop) = (origin.0.max(lo), end.min(hi));
        if start >= stop {
            return false;
        }
        // An edge kept keeps its float: `origin + size` gives `end` again.
        if start != origin.0 || stop != end {
            *size = ScaledPixels(stop - start);
            *origin = ScaledPixels(start);
        }
    }
    true
}

/// `rects` made pairwise disjoint: each one minus those before it, as up to
/// four bands. They cover the same pixels.
pub(super) fn disjoint(rects: &[Region]) -> Vec<[f32; 4]> {
    let mut out: Vec<[u32; 4]> = Vec::new();
    for r in rects {
        let mut parts = vec![[r.x, r.y, r.x + r.width, r.y + r.height]];
        for o in &out {
            parts = parts.into_iter().flat_map(|p| minus(p, *o)).collect();
        }
        out.extend(parts);
    }
    out.into_iter().map(|r| r.map(|v| v as f32)).collect()
}

/// `a` minus `b` (x0, y0, x1, y1): the bands of `a` above, below, left and
/// right of `b`.
fn minus(a: [u32; 4], b: [u32; 4]) -> Vec<[u32; 4]> {
    if a[0] >= b[2] || b[0] >= a[2] || a[1] >= b[3] || b[1] >= a[3] {
        return vec![a];
    }
    let (y0, y1) = (a[1].max(b[1]), a[3].min(b[3]));
    [
        [a[0], a[1], a[2], y0],
        [a[0], y1, a[2], a[3]],
        [a[0], y0, a[0].max(b[0]), y1],
        [a[2].min(b[2]), y0, a[2], y1],
    ]
    .into_iter()
    .filter(|p| p[0] < p[2] && p[1] < p[3])
    .collect()
}

/// Appends to `out` a copy of every instance of `items` per rectangle of
/// `rects` it reaches, its mask cut to the rectangle, in the order of
/// `items` (rectangles are disjoint, so every pixel still sees the batch's
/// order); returns how many and the pixels they cover.
pub(super) fn gather<T: Copy>(
    out: &mut Vec<T>,
    items: &[T],
    rects: &[[f32; 4]],
    extent: impl Fn(&T) -> Option<[f32; 4]>,
    mask: impl Fn(&mut T) -> &mut ContentMask<ScaledPixels>,
) -> (usize, f64) {
    let (start, mut area) = (out.len(), 0.0);
    for item in items {
        let e = extent(item);
        for r in rects.iter().filter(|r| meets(e, r)) {
            let mut copy = *item;
            if cut(mask(&mut copy), r) {
                if let Some(e) = e {
                    area += ((e[2].min(r[2]) - e[0].max(r[0])) * (e[3].min(r[3]) - e[1].max(r[1])))
                        .max(0.0) as f64;
                }
                out.push(copy);
            }
        }
    }
    (out.len() - start, area)
}

/// The rectangles of a pass as their bounding region (the scissor).
pub(super) fn bounds(rects: &[[f32; 4]]) -> Option<Region> {
    let first = *rects.first()?;
    let u = rects.iter().fold(first, |a, r| {
        [
            a[0].min(r[0]),
            a[1].min(r[1]),
            a[2].max(r[2]),
            a[3].max(r[3]),
        ]
    });
    let [x0, y0, x1, y1] = u.map(|v| v as u32);
    Some(Region {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

/// A rectangle of a pass as a region (for the batches drawn per rectangle).
pub(super) fn region(r: &[f32; 4]) -> Region {
    let [x0, y0, x1, y1] = r.map(|v| v as u32);
    Region {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }
}
