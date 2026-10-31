//! Индексы распорок и геометрии inline-коробок; порядок равных по размеру
//! коробок совпадает с исходным порядком их первых распорок.

use super::{Intervals, Paragraph};
use std::{collections::HashMap, ops::Range};

pub(in crate::text::paragraph) struct Boxes {
    pub(in crate::text::paragraph) extents: Intervals,
    pub(in crate::text::paragraph) order: Vec<usize>,
    pub(in crate::text::paragraph) bounds: Vec<(u32, usize, usize, usize, usize)>,
    ranks: HashMap<u32, usize>,
    offsets: Vec<usize>,
}

impl Boxes {
    pub(in crate::text::paragraph) fn new(p: &Paragraph) -> Self {
        let mut order: Vec<_> = (0..p.spacer_edges.len()).collect();
        order.sort_by_key(|&i| (p.spacer_edges[i].0, i));
        let offsets = order.iter().map(|&i| p.spacer_edges[i].0).collect();
        let mut source: Vec<(u32, usize, usize)> = Vec::new();
        let mut ranks = HashMap::new();
        for &(at, id, _, _) in &p.spacer_edges {
            let next = source.len();
            let i = *ranks.entry(id).or_insert_with(|| {
                source.push((id, at, at));
                next
            });
            source[i].1 = source[i].1.min(at);
            source[i].2 = source[i].2.max(at);
        }
        let mut extents = HashMap::new();
        for &(id, start, end) in &p.box_extents {
            extents.entry(id).or_insert((start, end));
        }
        let mut bounds: Vec<_> = source
            .into_iter()
            .map(|(id, lo, hi)| {
                let (start, end) = extents.get(&id).copied().unwrap_or((lo + 1, hi));
                let (lo, hi) = if extents.contains_key(&id) {
                    (lo.min(start), hi.max(end))
                } else {
                    (lo, hi)
                };
                (id, lo, hi, start, end)
            })
            .collect();
        bounds.sort_by_key(|b| b.2 - b.1);
        ranks = bounds.iter().enumerate().map(|(i, b)| (b.0, i)).collect();
        Self {
            extents: Intervals::new(p.box_extents.iter().map(|b| b.1..b.2)),
            order,
            bounds,
            ranks,
            offsets,
        }
    }

    pub(in crate::text::paragraph) fn edges(&self, range: Range<usize>) -> &[usize] {
        &self.order[self.offsets.partition_point(|&p| p < range.start)
            ..self.offsets.partition_point(|&p| p < range.end)]
    }

    pub(in crate::text::paragraph) fn line_boxes(
        &self,
        p: &Paragraph,
        range: Range<usize>,
    ) -> Vec<usize> {
        let mut ranks: Vec<_> = self
            .edges(range)
            .iter()
            .map(|&i| self.ranks[&p.spacer_edges[i].1])
            .collect();
        ranks.sort_unstable();
        ranks.dedup();
        ranks
    }

    pub(in crate::text::paragraph) fn own(
        &self,
        p: &Paragraph,
        at: usize,
        id: u32,
    ) -> Option<(bool, bool)> {
        self.edges(at..at + 1)
            .iter()
            .map(|&i| p.spacer_edges[i])
            .find(|e| e.1 == id)
            .map(|e| (e.2, e.3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::paragraph::{Align, Wrap};
    use gpui::px;

    #[test]
    fn box_order_extent_and_duplicate_edge_selection_are_stable() {
        let mut p = Paragraph::new(
            "abcdefghijkl".into(),
            vec![],
            px(16.),
            px(20.),
            Align::Left,
            Wrap::default(),
        );
        p.spacer_edges = vec![
            (8, 1, false, false),
            (2, 2, true, false),
            (3, 2, false, false),
            (0, 1, true, false),
            (2, 2, false, true),
        ];
        p.box_extents = vec![(1, 1, 9), (1, 0, 12), (2, 2, 4)];
        let index = Boxes::new(&p);
        assert_eq!(index.bounds, vec![(2, 2, 4, 2, 4), (1, 0, 9, 1, 9)]);
        assert_eq!(index.edges(2..4), &[1, 4, 2]);
        assert_eq!(index.own(&p, 2, 2), Some((true, false)));
        assert_eq!(index.line_boxes(&p, 0..9), vec![0, 1]);
        assert_eq!(index.line_boxes(&p, 4..8), Vec::<usize>::new());
    }
}
