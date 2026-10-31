//! Индекс визуальных соседей строки: диапазон логических байтов ищется
//! за O(log n), невидимые управляющие знаки пропускаются при подготовке.

use super::super::{Paragraph, bidi_control};
use std::ops::Range;

pub(crate) struct VisualOrder {
    bytes: Vec<usize>,
    extrema: Vec<(usize, usize)>,
    left: Vec<Option<usize>>,
    right: Vec<Option<usize>>,
    size: usize,
}

impl VisualOrder {
    pub(in crate::text::paragraph) fn new(
        p: &Paragraph,
        pieces: impl Iterator<Item = (usize, usize, bool)>,
    ) -> Self {
        let visual = p.visual_chars(pieces);
        let mut logical: Vec<_> = visual.iter().copied().zip(0..).collect();
        logical.sort_unstable_by_key(|p| p.0);
        let size = logical.len().next_power_of_two();
        let mut extrema = vec![(usize::MAX, 0); 2 * size];
        for (i, &(_, rank)) in logical.iter().enumerate() {
            extrema[size + i] = (rank, rank);
        }
        for i in (1..size).rev() {
            extrema[i] = merge(extrema[2 * i], extrema[2 * i + 1]);
        }
        let visible = |at| !p.text[at..].chars().next().is_some_and(bidi_control);
        let mut left = vec![None; visual.len()];
        let mut previous = None;
        for (i, &at) in visual.iter().enumerate() {
            left[i] = previous;
            if visible(at) {
                previous = Some(at);
            }
        }
        let mut right = vec![None; visual.len()];
        let mut next = None;
        for (i, &at) in visual.iter().enumerate().rev() {
            right[i] = next;
            if visible(at) {
                next = Some(at);
            }
        }
        Self {
            bytes: logical.into_iter().map(|p| p.0).collect(),
            extrema,
            left,
            right,
            size,
        }
    }

    pub(in crate::text::paragraph) fn neighbours(
        &self,
        range: &Range<usize>,
    ) -> (Option<usize>, Option<usize>) {
        let mut a = self.size + self.bytes.partition_point(|b| *b < range.start);
        let mut b = self.size + self.bytes.partition_point(|b| *b < range.end);
        let mut result = (usize::MAX, 0);
        while a < b {
            if a % 2 == 1 {
                result = merge(result, self.extrema[a]);
                a += 1;
            }
            if b % 2 == 1 {
                b -= 1;
                result = merge(result, self.extrema[b]);
            }
            a /= 2;
            b /= 2;
        }
        if result.0 == usize::MAX {
            (None, None)
        } else {
            (self.left[result.0], self.right[result.1])
        }
    }
}

fn merge(a: (usize, usize), b: (usize, usize)) -> (usize, usize) {
    (a.0.min(b.0), a.1.max(b.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::paragraph::{Align, Wrap, visual_neighbours};
    use gpui::px;

    #[test]
    fn indexed_neighbours_match_visual_scan_including_controls() {
        let text = "a\u{202e}אב\u{202c} cd";
        let p = Paragraph::new(
            text.into(),
            vec![],
            px(16.),
            px(20.),
            Align::Left,
            Wrap::default(),
        );
        let (_, pieces) = p.resolved_visual(&(0..text.len()), false).unwrap();
        let visual = p.visual_chars(pieces.iter().copied());
        let index = VisualOrder::new(&p, pieces.iter().copied());
        for a in (0..=text.len()).filter(|&i| text.is_char_boundary(i)) {
            for b in (a..=text.len()).filter(|&i| text.is_char_boundary(i)) {
                assert_eq!(
                    index.neighbours(&(a..b)),
                    visual_neighbours(text, &visual, &(a..b))
                );
            }
        }
        let empty = VisualOrder::new(&p, std::iter::empty());
        assert_eq!(empty.neighbours(&(0..text.len())), (None, None));
    }
}
