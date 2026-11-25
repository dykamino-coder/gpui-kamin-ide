//! KaminIDE patch: what a frame comparison found different, by primitive kind
//! (DevTools `why`): the counters that name why a scroll step or an edit was
//! drawn whole instead of guessing it from the page.

use super::super::KIND_SHIFT;
use super::items::Item;

/// Kinds counted apart: the eight primitive kinds and the unchanged-layer marker.
pub const SHIFT_KINDS: usize = 9;

/// Kind names in `ShiftStats::differs` order.
pub const SHIFT_KIND_NAMES: [&str; SHIFT_KINDS] = [
    "shadow",
    "quad",
    "path",
    "underline",
    "mono",
    "subpixel",
    "poly",
    "surface",
    "layer",
];

/// What one comparison found; it stops early, so counts are at least these.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShiftStats {
    /// Cells compared (inside the copied window part).
    pub cells: u32,
    /// Cells whose new and old item counts differ (damaged without comparing).
    pub count_cells: u32,
    /// Differing pairs by the new item's kind (`SHIFT_KIND_NAMES`).
    pub differs: [u32; SHIFT_KINDS],
    /// The first differing pair: new kind, old kind, the union of their areas.
    pub first: Option<(u8, u8, [f32; 4])>,
    /// CPU time of the comparison, microseconds.
    pub us: u32,
    /// Groups of a scroll step (`groups::GROUP_COUNTS`), all 0 in place.
    pub groups: [u32; super::groups::GROUP_COUNTS],
}

fn kind(item: &Item) -> u8 {
    if item.marker != 0 {
        8
    } else {
        (item.entry >> KIND_SHIFT) as u8
    }
}

impl ShiftStats {
    pub(super) fn differ(&mut self, x: &Item, y: &Item, area: [f32; 4]) {
        self.differs[kind(x) as usize] += 1;
        self.first.get_or_insert((kind(x), kind(y), area));
    }
}

impl std::fmt::Display for ShiftStats {
    /// `cells 120 count 3 differ quad 40 poly 2 first quad/quad [x0,y0,x1,y1]`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cells {} count {} us {}",
            self.cells, self.count_cells, self.us
        )?;
        let shown = self
            .differs
            .iter()
            .zip(SHIFT_KIND_NAMES)
            .filter(|(n, _)| **n > 0);
        for (i, (n, name)) in shown.enumerate() {
            write!(f, "{}{name} {n}", if i == 0 { " differ " } else { " " })?;
        }
        if self.groups[0] > 0 {
            let [n, base, fixed, nokey, differ, moved] = self.groups;
            write!(
                f,
                " groups {n} in_place {base} unmovable {fixed} nokey {nokey} differ {differ} moved {moved}"
            )?;
        }
        if let Some((a, b, r)) = self.first {
            let (a, b) = (SHIFT_KIND_NAMES[a as usize], SHIFT_KIND_NAMES[b as usize]);
            write!(
                f,
                " first {a}/{b} [{:.0},{:.0},{:.0},{:.0}]",
                r[0], r[1], r[2], r[3]
            )?;
        }
        Ok(())
    }
}
