//! KaminIDE patch: the damage of a moved frame as a few rectangles sized by
//! what changed (`plan_shift`).
//!
//! Damaged parts of cells are united per row of cells into one span, rows
//! with a span stack into rectangles, and the cheapest unions bring them to
//! the limit: a strip uncovered by the step, a fixed bar and a scroll thumb
//! stay three rectangles of their own size, not the cells of a grid.

use super::super::DAMAGE_TILE;

/// Damage of one frame: per row of cells the union of its parts (x0, y0, x1, y1).
pub(super) struct Damage {
    w: u32,
    h: u32,
    rows: Vec<Option<[u32; 4]>>,
    /// Whole bands outside the moved window.
    bands: Vec<[u32; 4]>,
    /// Area of the row unions and the bands: they are disjoint, and every
    /// rectangle set inish returns covers them, so this bounds its area below.
    total: u64,
}

fn area(r: &[u32; 4]) -> u64 {
    (r[2] - r[0]) as u64 * (r[3] - r[1]) as u64
}

fn union(a: &[u32; 4], b: &[u32; 4]) -> [u32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

impl Damage {
    pub(super) fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            rows: vec![None; h.div_ceil(DAMAGE_TILE) as usize],
            bands: Vec::new(),
            total: 0,
        }
    }

    /// A damaged rectangle inside one row of cells.
    pub(super) fn add(&mut self, r: [u32; 4]) {
        let row = &mut self.rows[(r[1] / DAMAGE_TILE) as usize];
        let before = row.as_ref().map_or(0, area);
        let after = row.map_or(r, |u| union(&u, &r));
        *row = Some(after);
        self.total += area(&after) - before;
    }

    /// Everything outside `copy` (x, y, w, h): the moved pixels do not reach it.
    pub(super) fn uncovered(&mut self, copy: [u32; 4]) {
        let (x0, y0, x1, y1) = (copy[0], copy[1], copy[0] + copy[2], copy[1] + copy[3]);
        let (w, h) = (self.w, self.h);
        for band in [
            [0, 0, w, y0],
            [0, y1, w, h],
            [0, y0, x0, y1],
            [x1, y0, w, y1],
        ] {
            if band[0] < band[2] && band[1] < band[3] {
                self.total += area(&band);
                self.bands.push(band);
            }
        }
    }

    /// Pixels damaged so far, counted once (see `total`).
    pub(super) fn area(&self) -> u64 {
        self.total
    }

    /// At most `limit` rectangles (x, y, w, h) covering the damage.
    pub(super) fn finish(self, limit: usize) -> Vec<[u32; 4]> {
        let mut rects: Vec<[u32; 4]> = Vec::new();
        for r in self.rows.into_iter().flatten() {
            // A row continues the rectangle above when their spans meet.
            match rects.last_mut() {
                Some(last)
                    if last[3] >= r[1]
                        && last[0] <= r[2]
                        && r[0] <= last[2]
                        && area(&union(last, &r)) <= area(last) + area(&r) * 2 =>
                {
                    *last = union(last, &r);
                }
                _ => rects.push(r),
            }
        }
        rects.extend(self.bands);
        // Merge the pair whose union wastes the least until within the limit.
        while rects.len() > limit {
            let mut best = (u64::MAX, 0, 1);
            for i in 0..rects.len() {
                for j in i + 1..rects.len() {
                    let u = union(&rects[i], &rects[j]);
                    let waste = area(&u).saturating_sub(area(&rects[i]) + area(&rects[j]));
                    if waste < best.0 {
                        best = (waste, i, j);
                    }
                }
            }
            let (_, i, j) = best;
            let b = rects.swap_remove(j);
            rects[i] = union(&rects[i], &b);
        }
        // Rectangles that came to overlap by a merge are united once more.
        let mut out: Vec<[u32; 4]> = Vec::new();
        for r in rects {
            match out.iter_mut().find(|o| {
                o[0] < r[2]
                    && r[0] < o[2]
                    && o[1] < r[3]
                    && r[1] < o[3]
                    && area(&union(o, &r)) <= area(o) + area(&r)
            }) {
                Some(o) => *o = union(o, &r),
                None => out.push(r),
            }
        }
        out.into_iter()
            .map(|r| [r[0], r[1], r[2] - r[0], r[3] - r[1]])
            .collect()
    }
}
