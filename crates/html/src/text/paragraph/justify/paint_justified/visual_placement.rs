//! Visual placement for paint_justified; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{Pixels, px};

impl Paragraph {
    pub(crate) fn place_visual(
        &self,
        ltr_runs: &[(usize, usize, bool)],
        logical_at: &dyn Fn(usize) -> Pixels,
    ) -> Vec<(usize, usize, bool, Pixels)> {
        let mut ltr_place: Vec<(usize, usize, bool, Pixels)> = Vec::new();
        if !ltr_runs.is_empty() {
            let mut units: Vec<(usize, usize, bool)> = Vec::new();
            for &(s, e, rtl) in ltr_runs.iter() {
                let mut pts = vec![s, e];
                for &p in self.spacers.iter().filter(|p| **p >= s && **p < e) {
                    pts.push(p);
                    pts.push((p + crate::text::inline::SPACER.len()).min(e));
                }
                // Box content edges: a fragment of a box starts a piece.
                for b in &self.box_extents {
                    pts.extend([b.1, b.2].into_iter().filter(|p| *p > s && *p < e));
                }
                pts.sort_unstable();
                pts.dedup();
                let mut run: Vec<(usize, usize, bool)> =
                    pts.windows(2).map(|w| (w[0], w[1], rtl)).collect();
                if rtl {
                    run.reverse();
                }
                units.extend(run);
            }
            // CSS 2.1 §8.6 / Blink `UpdateFragmentEdges`: a box split by
            // reordering keeps its left edge on its leftmost fragment and its
            // right edge on the rightmost one. Inner boxes first.
            let mut boxes: Vec<(u32, usize, usize)> = Vec::new();
            for &(p, id, _, _) in &self.spacer_edges {
                match boxes.iter_mut().find(|b| b.0 == id) {
                    Some(b) => {
                        b.1 = b.1.min(p);
                        b.2 = b.2.max(p);
                    }
                    None => boxes.push((id, p, p)),
                }
            }
            for b in boxes.iter_mut() {
                if let Some(x) = self.box_extents.iter().find(|x| x.0 == b.0) {
                    b.1 = b.1.min(x.1);
                    b.2 = b.2.max(x.2);
                }
            }
            boxes.sort_by_key(|b| b.2 - b.1);
            for &(id, lo, hi) in &boxes {
                let own = |a: usize| {
                    self.spacer_edges
                        .iter()
                        .find(|e| e.0 == a && e.1 == id)
                        .map(|e| (e.2, e.3))
                };
                if !units.iter().any(|u| own(u.0).is_some()) {
                    continue;
                }
                let rest: Vec<(usize, usize, bool)> = units
                    .iter()
                    .copied()
                    .filter(|u| own(u.0).is_none())
                    .collect();
                // Content of the box: between its markers when known, else
                // strictly between its own edge spacers.
                let (from_at, to_at) = self
                    .box_extents
                    .iter()
                    .find(|b| b.0 == id)
                    .map_or((lo + 1, hi), |b| (b.1, b.2));
                let inside: Vec<usize> = rest
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.0 >= from_at && u.0 < to_at && u.0 < u.1)
                    .map(|(i, _)| i)
                    .collect();
                let (Some(&first), Some(&last)) = (inside.first(), inside.last()) else {
                    continue;
                };
                // Outermost spacer (the margin) goes furthest out: logical
                // order already is visual for an ltr parent, reversed for rtl.
                let mut lefts: Vec<(usize, usize, bool)> = Vec::new();
                let mut rights: Vec<(usize, usize, bool)> = Vec::new();
                let mut parent_rtl = false;
                for &(a, b, rtl) in units.iter() {
                    if let Some((left, prtl)) = own(a) {
                        parent_rtl = prtl;
                        if left {
                            lefts.push((a, b, rtl));
                        } else {
                            rights.push((a, b, rtl));
                        }
                    }
                }
                lefts.sort_by_key(|u| u.0);
                rights.sort_by_key(|u| u.0);
                if parent_rtl {
                    lefts.reverse();
                    rights.reverse();
                }
                let mut next = rest;
                next.splice(last + 1..last + 1, rights);
                next.splice(first..first, lefts);
                units = next;
            }
            let mut cursor = px(0.);
            for (a, b, rtl) in units {
                ltr_place.push((a, b, rtl, cursor));
                cursor += logical_at(b) - logical_at(a);
            }
        }
        ltr_place
    }
}
