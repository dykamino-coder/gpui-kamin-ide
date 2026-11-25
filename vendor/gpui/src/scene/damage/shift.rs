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

use super::{Area, DAMAGE_TILE, KIND_SHIFT, entry_area};
use crate::scene::Scene;
use collections::FxHashMap;
use items::{Item, bin, each_primitive, items, unchanged_layers};

mod compare;
#[cfg(test)]
mod group_tests;
mod items;
#[cfg(test)]
mod layer_tests;
mod rects;
mod stats;
#[cfg(test)]
mod tests;
pub use stats::{SHIFT_KIND_NAMES, SHIFT_KINDS, ShiftStats};

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

/// `W`: the part of a `w`×`h` window that the frame moved by `d` covers.
fn moved_window(w: u32, h: u32, d: (i32, i32)) -> Option<[u32; 4]> {
    let span = |n: u32, d: i32| {
        let (lo, hi) = (d.max(0), (n as i32 + d).min(n as i32));
        (hi > lo).then_some((lo as u32, (hi - lo) as u32))
    };
    let ((x, width), (y, height)) = (span(w, d.0)?, span(h, d.1)?);
    Some([x, y, width, height])
}

impl Scene {
    /// The primitives that may write device pixel (`x`, `y`), in paint order:
    /// kind, index and area (the renderer's damage audit names them).
    #[doc(hidden)]
    pub fn primitives_at(&self, x: f32, y: f32) -> Vec<String> {
        let mut out = Vec::new();
        let mask = (1 << KIND_SHIFT) - 1;
        each_primitive(self, &|slot| self.layer(slot), &mut |s, entry| {
            if let Some(a) = entry_area(s, entry)
                && a[0] <= x + 1.0
                && a[1] <= y + 1.0
                && a[2] >= x
                && a[3] >= y
            {
                out.push(format!("k{}#{} {a:?}", entry >> KIND_SHIFT, entry & mask));
            }
        });
        out
    }

    /// `plan_shift` within the comparison budget (`shift_budget_us`), with its
    /// CPU time in `stats.us`; `groups` matches this frame's groups to the
    /// last frame's (`Scene::group_base`, plus one; 0: none).
    pub(super) fn timed_plan_shift(
        &self,
        previous: &Scene,
        size: (u32, u32),
        d: (i32, i32),
        groups: &[u32],
        stats: &mut ShiftStats,
    ) -> Result<FrameShift, &'static str> {
        let started = std::time::Instant::now();
        let budget = super::shift_budget_us();
        let deadline =
            (budget != u32::MAX).then(|| started + std::time::Duration::from_micros(budget as u64));
        let plan = self.plan_shift_within(previous, size, d, groups, stats, deadline);
        stats.us = started.elapsed().as_micros().min(u32::MAX as u128) as u32;
        plan
    }

    /// The frame drawn as `previous` moved by `d` device pixels, with what
    /// then needs a raster; `Err` names why not: the comparison cannot run,
    /// or the redraw would cover about the whole window anyway.
    /// `stats` counts what the comparison found different.
    pub(super) fn plan_shift(
        &self,
        previous: &Scene,
        size: (u32, u32),
        d: (i32, i32),
        stats: &mut ShiftStats,
    ) -> Result<FrameShift, &'static str> {
        self.plan_shift_within(previous, size, d, &[], stats, None)
    }

    /// `plan_shift` that gives up (`over_budget`) once `deadline` passes: the
    /// comparison then costs more than the raster it could save.
    fn plan_shift_within(
        &self,
        previous: &Scene,
        size: (u32, u32),
        d: (i32, i32),
        groups: &[u32],
        stats: &mut ShiftStats,
        deadline: Option<std::time::Instant>,
    ) -> Result<FrameShift, &'static str> {
        let late = || deadline.is_some_and(|t| std::time::Instant::now() >= t);
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
        // In place, layers the engine recorded again unchanged are equal as a whole.
        let unchanged = if d == (0, 0) {
            unchanged_layers(self, previous)
        } else {
            Vec::new()
        };
        let new = items(self, &|slot| self.layer(slot), &unchanged, (0.0, 0.0), wa)?;
        let old = items(
            previous,
            &|slot| self.drawn_layer(previous, slot),
            &unchanged,
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
        if late() {
            return Err("over_budget");
        }
        let mut cmp = compare::Moved {
            d: (d.0 as f32, d.1 as f32),
            w: wa,
            groups,
            exact: FxHashMap::default(),
        };
        let mut damage = rects::Damage::new(w, h);
        damage.uncovered(copy);
        // The redraw only grows from here: stop once it is about the whole window.
        let whole = |damage: &rects::Damage| damage.area() * 20 >= w as u64 * h as u64 * 17;
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
            stats.cells += 1;
            if late() {
                return Err("over_budget");
            }
            let (a, b) = (
                &ea[sa[t] as usize..sa[t + 1] as usize],
                &eb[sb[t] as usize..sb[t + 1] as usize],
            );
            if a.len() != b.len() {
                stats.count_cells += 1;
                damage.add(p);
                if whole(&damage) {
                    return Err("redraw_whole");
                }
                continue;
            }
            for (&x, &y) in a.iter().zip(b) {
                let (x, y) = ((x, new[x as usize]), (y, old[y as usize]));
                let differs = match x.1.same_marker(&y.1) {
                    Some(same) => !same,
                    None => {
                        x.1.entry >> KIND_SHIFT != y.1.entry >> KIND_SHIFT || !cmp.same(x, y, p)
                    }
                };
                if differs {
                    let u = [
                        x.1.area[0].min(y.1.area[0]),
                        x.1.area[1].min(y.1.area[1]),
                        x.1.area[2].max(y.1.area[2]),
                        x.1.area[3].max(y.1.area[3]),
                    ];
                    stats.differ(&x.1, &y.1, u);
                    let q = [
                        (u[0].floor().max(p[0] as f32)) as u32,
                        (u[1].floor().max(p[1] as f32)) as u32,
                        (u[2].ceil().min(p[2] as f32)) as u32,
                        (u[3].ceil().min(p[3] as f32)) as u32,
                    ];
                    if q[0] < q[2] && q[1] < q[3] {
                        damage.add(q);
                        if whole(&damage) {
                            return Err("redraw_whole");
                        }
                    }
                }
            }
        }
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
