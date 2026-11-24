//! KaminIDE patch: a scroll step as a move of the frame drawn last.
//!
//! A wheel step moves most of the frame by a whole number of device pixels
//! `d`. A renderer that keeps its target can copy the kept pixels by `d` and
//! raster only where the new frame differs from that copy. The copy is the
//! last frame translated: a virtual scene `T` with every primitive of the last
//! frame moved by `d`. Its pixels inside `W` (the window ∩ the window moved by
//! `d`) are the copied ones, since rasterization does not depend on where a
//! primitive lies when it moves by whole pixels (masks clip at pixel centres,
//! edges antialias by distance). So the frame redraws exactly where `S`, the
//! new frame, differs from `T` within `W`, plus all outside `W` (the strip the
//! step uncovered). Cells are compared as in the frame damage, by their
//! primitive sequences, with retained layers expanded in their paint order
//! and both masks cut by `W` (pixels outside `W` are redrawn anyway).
//!
//! What does not move with the content is found, not declared: a fixed box
//! or a scroll bar differs between `S` and `T` and damages its own cells; a
//! plain fill (no border, radius, transform or gradient) covering a cell in
//! both is equal there, so the page background under a scroller costs nothing.
//! Cell for cell this is the comparison WebRender runs per picture tile
//! (`update_dirty_rects`), with the frame itself as the retained surface: no
//! slice per scroll root, no tile left half invalid, and the copied pixels are
//! the frame's own (text over its real backdrop), so the result equals a
//! whole redraw bit for bit.

use super::{Area, DAMAGE_TILE, FULL, KIND_SHIFT, entry_area, paint_order};
use crate::scene::{Scene, SceneLayer};
use collections::FxHashMap;

mod compare;
mod rects;
#[cfg(test)]
mod tests;

/// Most rectangles a moved frame rasters after the copy.
pub const SHIFT_RECTS: usize = 4;

/// A frame drawn as the last one moved, then redrawn within rectangles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameShift {
    /// Device pixels the last frame's pixels move by.
    pub delta: (i32, i32),
    /// Where the moved pixels land: x, y, width, height (device px).
    pub copy: [u32; 4],
    /// Rectangles rasterized after the move (x, y, width, height, device px),
    /// at most `SHIFT_RECTS`, disjoint from none in particular.
    pub redraw: Vec<[u32; 4]>,
}

impl FrameShift {
    /// Pixels the redraw rectangles cover (counted per rectangle).
    pub fn redraw_area(&self) -> u64 {
        self.redraw.iter().map(|r| r[2] as u64 * r[3] as u64).sum()
    }

    /// Whether the pixels stay where they are (a frame compared in place).
    pub fn in_place(&self) -> bool {
        self.delta == (0, 0)
    }

    /// The bounding rectangle of the pixels that change (x, y, w, h): the
    /// copy and every redraw, or only the redraws of a frame in place.
    pub fn bounds(&self) -> [u32; 4] {
        let copy = (!self.in_place()).then_some(self.copy);
        let all = copy.into_iter().chain(self.redraw.iter().copied());
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for r in all.filter(|r| r[2] > 0 && r[3] > 0) {
            (x0, y0) = (x0.min(r[0]), y0.min(r[1]));
            (x1, y1) = (x1.max(r[0] + r[2]), y1.max(r[1] + r[3]));
        }
        if x1 <= x0 {
            [0; 4]
        } else {
            [x0, y0, x1 - x0, y1 - y0]
        }
    }
}

/// One primitive in paint order: its scene, entry (`KIND_SHIFT` encoding)
/// and the area it may write, cut by `W`.
#[derive(Clone, Copy)]
pub(super) struct Item<'a> {
    pub(super) scene: &'a Scene,
    pub(super) entry: u32,
    pub(super) area: Area,
}

/// Every primitive of `scene` in raster order, the content of its layers
/// (`layer(slot)`) at the layer's place; `false` when a layer's content is
/// unknown or holds layers itself.
fn walk<'a>(
    scene: &'a Scene,
    layer: &dyn Fn(usize) -> Option<&'a SceneLayer>,
    each: &mut dyn FnMut(&'a Scene, u32),
) -> bool {
    let slots = &scene.book.layer_order;
    if slots.is_empty() {
        paint_order(scene, |e| each(scene, e));
        return true;
    }
    // Layers are drawn as one batch at their order, after the primitives of
    // the same order (`PrimitiveKind::Layer` is the last kind).
    let mut own = Vec::new();
    paint_order(scene, |e| own.push(e));
    let order_of = |e: u32| super::entry_order(scene, e);
    let mut at = 0;
    for &slot in slots {
        let key = scene.book.layers[slot as usize].order;
        while at < own.len() && order_of(own[at]) <= key {
            each(scene, own[at]);
            at += 1;
        }
        let Some(content) = layer(slot as usize) else {
            return false;
        };
        if !content.scene.book.layers.is_empty() {
            return false;
        }
        paint_order(&content.scene, |e| each(&content.scene, e));
    }
    own[at..].iter().for_each(|&e| each(scene, e));
    true
}

/// `W`: the part of a `w`×`h` window that the frame moved by `d` covers.
fn moved_window(w: u32, h: u32, d: (i32, i32)) -> Option<[u32; 4]> {
    let span = |n: u32, d: i32| {
        let (lo, hi) = (d.max(0), (n as i32 + d).min(n as i32));
        (hi > lo).then_some((lo as u32, (hi - lo) as u32))
    };
    let ((x, width), (y, height)) = (span(w, d.0)?, span(h, d.1)?);
    Some([x, y, width, height])
}

/// The items of `scene` (moved by `d`) that write inside `w`; `Err` when a
/// primitive reads the frame (a backdrop) or a layer's content is unknown.
fn items<'a>(
    scene: &'a Scene,
    layer: &dyn Fn(usize) -> Option<&'a SceneLayer>,
    d: (f32, f32),
    w: Area,
) -> Result<Vec<Item<'a>>, &'static str> {
    let mut out = Vec::new();
    let mut backdrop = !scene.hoisted_backdrops.is_empty();
    let complete = walk(scene, layer, &mut |s, entry| {
        let Some(a) = entry_area(s, entry) else {
            backdrop = true;
            return;
        };
        let a = if a.iter().any(|v| v.is_nan()) {
            FULL
        } else {
            a
        };
        let a = [
            (a[0] + d.0).max(w[0]),
            (a[1] + d.1).max(w[1]),
            (a[2] + d.0).min(w[2]),
            (a[3] + d.1).min(w[3]),
        ];
        if a[0] < a[2] && a[1] < a[3] {
            out.push(Item {
                scene: s,
                entry,
                area: a,
            });
        }
    });
    match (complete, backdrop) {
        (false, _) => Err("layer_unknown"),
        (_, true) => Err("backdrop"),
        _ => Ok(out),
    }
}

/// Cells of a `cols`×`rows` grid with the items that may write in each (CSR).
fn bin(items: &[Item], cols: u32, rows: u32) -> (Vec<u32>, Vec<u32>) {
    let tile = DAMAGE_TILE as f32;
    let span = |a: &Area| {
        let c0 = (a[0] / tile).floor().clamp(0.0, (cols - 1) as f32) as u32;
        let r0 = (a[1] / tile).floor().clamp(0.0, (rows - 1) as f32) as u32;
        let c1 = ((a[2] / tile).ceil().clamp(1.0, cols as f32) as u32).max(c0 + 1);
        let r1 = ((a[3] / tile).ceil().clamp(1.0, rows as f32) as u32).max(r0 + 1);
        (c0, r0, c1, r1)
    };
    let mut starts = vec![0u32; (cols * rows) as usize + 1];
    for item in items {
        let (c0, r0, c1, r1) = span(&item.area);
        for r in r0..r1 {
            for c in c0..c1 {
                starts[(r * cols + c) as usize + 1] += 1;
            }
        }
    }
    for t in 0..(cols * rows) as usize {
        starts[t + 1] += starts[t];
    }
    let mut fill = starts.clone();
    let mut entries = vec![0u32; starts[(cols * rows) as usize] as usize];
    for (k, item) in items.iter().enumerate() {
        let (c0, r0, c1, r1) = span(&item.area);
        for r in r0..r1 {
            for c in c0..c1 {
                let t = (r * cols + c) as usize;
                entries[fill[t] as usize] = k as u32;
                fill[t] += 1;
            }
        }
    }
    (starts, entries)
}

impl Scene {
    /// The frame drawn as `previous` moved by `d` device pixels, with what
    /// then needs a raster; `Err` names why not: the comparison cannot run,
    /// or the redraw would cover about the whole window anyway.
    pub(super) fn plan_shift(
        &self,
        previous: &Scene,
        size: (u32, u32),
        d: (i32, i32),
    ) -> Result<FrameShift, &'static str> {
        let (w, h) = size;
        // `d == (0, 0)` compares the frame with the last one in place: the
        // frame damage when the retained layers' bookkeeping cannot tell it.
        if w == 0 || h == 0 {
            return Err("no_step");
        }
        let copy = moved_window(w, h, d).ok_or("step_beyond_window")?;
        let wa: Area = [
            copy[0] as f32,
            copy[1] as f32,
            (copy[0] + copy[2]) as f32,
            (copy[1] + copy[3]) as f32,
        ];
        let new = items(self, &|slot| self.layer(slot), (0.0, 0.0), wa)?;
        let old = items(
            previous,
            &|slot| self.drawn_layer(previous, slot),
            (d.0 as f32, d.1 as f32),
            wa,
        )
        .map_err(|why| {
            if why == "layer_unknown" {
                "last_layer_unknown"
            } else {
                why
            }
        })?;
        let (cols, rows) = (w.div_ceil(DAMAGE_TILE), h.div_ceil(DAMAGE_TILE));
        let (sa, ea) = bin(&new, cols, rows);
        let (sb, eb) = bin(&old, cols, rows);
        let mut cmp = compare::Moved {
            d: (d.0 as f32, d.1 as f32),
            w: wa,
            exact: FxHashMap::default(),
        };
        let mut damage = rects::Damage::new(w, h);
        for t in 0..(cols * rows) as usize {
            let (c, r) = (t as u32 % cols, t as u32 / cols);
            let cell = [
                c * DAMAGE_TILE,
                r * DAMAGE_TILE,
                ((c + 1) * DAMAGE_TILE).min(w),
                ((r + 1) * DAMAGE_TILE).min(h),
            ];
            let p = [
                cell[0].max(copy[0]),
                cell[1].max(copy[1]),
                cell[2].min(copy[0] + copy[2]),
                cell[3].min(copy[1] + copy[3]),
            ];
            if p[0] >= p[2] || p[1] >= p[3] {
                continue;
            }
            let (a, b) = (
                &ea[sa[t] as usize..sa[t + 1] as usize],
                &eb[sb[t] as usize..sb[t + 1] as usize],
            );
            if a.len() != b.len() {
                damage.add(p);
                continue;
            }
            for (&x, &y) in a.iter().zip(b) {
                let (x, y) = ((x, new[x as usize]), (y, old[y as usize]));
                if x.1.entry >> KIND_SHIFT != y.1.entry >> KIND_SHIFT || !cmp.same(x, y, p) {
                    let u = [
                        x.1.area[0].min(y.1.area[0]),
                        x.1.area[1].min(y.1.area[1]),
                        x.1.area[2].max(y.1.area[2]),
                        x.1.area[3].max(y.1.area[3]),
                    ];
                    let q = [
                        (u[0].floor().max(p[0] as f32)) as u32,
                        (u[1].floor().max(p[1] as f32)) as u32,
                        (u[2].ceil().min(p[2] as f32)) as u32,
                        (u[3].ceil().min(p[3] as f32)) as u32,
                    ];
                    if q[0] < q[2] && q[1] < q[3] {
                        damage.add(q);
                    }
                }
            }
        }
        damage.uncovered(copy);
        let redraw = damage.finish(SHIFT_RECTS);
        let shift = FrameShift {
            delta: d,
            copy,
            redraw,
        };
        // Copy and raster cost more than a whole raster only when the raster
        // is about the whole window.
        (shift.redraw_area() * 20 < w as u64 * h as u64 * 17)
            .then_some(shift)
            .ok_or("redraw_whole")
    }
}
