//! Видимые куски строки слева направо (`place_visual`): каждый прогон
//! уровня в порядке UAX #9 L2, разрезанный по распоркам краёв и краям
//! содержимого коробок; коробка, разбитая перестановкой, держит левый край
//! на самом левом фрагменте и правый — на самом правом (CSS 2.1 §8.6, Blink
//! `UpdateFragmentEdges`), внешняя распорка (поле) уходит дальше всех.

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
                let edges = &self.prepared().box_edges;
                pts.extend_from_slice(
                    &edges[edges.partition_point(|p| *p <= s)..edges.partition_point(|p| *p < e)],
                );
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
            let index = &self.prepared().box_index;
            let start = ltr_runs.iter().map(|r| r.0).min().unwrap();
            let end = ltr_runs.iter().map(|r| r.1).max().unwrap();
            for rank in index.line_boxes(self, start..end) {
                let (id, _, _, from_at, to_at) = index.bounds[rank];
                let own = |a| index.own(self, a, id);
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
                // order already is visual for an ltr box, reversed for rtl.
                let mut lefts: Vec<(usize, usize, bool)> = Vec::new();
                let mut rights: Vec<(usize, usize, bool)> = Vec::new();
                let mut own_rtl = false;
                for &(a, b, rtl) in units.iter() {
                    if let Some((left, prtl)) = own(a) {
                        own_rtl = prtl;
                        if left {
                            lefts.push((a, b, rtl));
                        } else {
                            rights.push((a, b, rtl));
                        }
                    }
                }
                lefts.sort_by_key(|u| u.0);
                rights.sort_by_key(|u| u.0);
                if own_rtl {
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
