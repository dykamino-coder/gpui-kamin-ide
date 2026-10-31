//! Подготовленные данные одной ревизии абзаца: границы прогонов, индексы
//! стилей, разрешённые уровни bidi и неизменная часть ключей замера.

mod bidi;
mod boxes;
mod intervals;
mod justification;
pub(super) mod paint_runs;
mod positions;
mod runs;
#[cfg(test)]
mod tests;
pub(super) mod visual_order;

use super::Paragraph;
use bidi::ResolvedBidi;
use intervals::Intervals;
use std::{cell::OnceCell, ops::Range};
use unicode_bidi::Level;

pub(super) struct Prepared {
    pub(super) run_ends: Vec<usize>,
    pub(super) box_index: boxes::Boxes,
    pub(super) has_bands: bool,
    pub(super) has_metric_overrides: bool,
    pub(super) letters: Intervals,
    pub(super) words: Intervals,
    pub(super) shifts: Intervals,
    pub(super) boxes: Intervals,
    pub(super) edges: Intervals,
    pub(super) relatives: Intervals,
    pub(super) run_edges: Vec<Option<(usize, usize)>>,
    pub(super) cuts: Vec<usize>,
    pub(super) box_edges: Vec<usize>,
    pub(super) decorations: Intervals,
    pub(super) emphases: Intervals,
    pub(super) line_heights: Intervals,
    pub(super) atoms: Intervals,
    pub(super) measure_key: OnceCell<u64>,
    pub(super) split_key: OnceCell<u64>,
    pub(super) joined: OnceCell<bool>,
    pub(super) opportunities: justification::Opportunities,
    bidi: OnceCell<ResolvedBidi>,
    rtl_bidi: OnceCell<ResolvedBidi>,
    neutral_bidi: OnceCell<(String, ResolvedBidi)>,
}

impl Prepared {
    fn new(p: &Paragraph) -> Self {
        let mut at = 0;
        let run_ends: Vec<_> = p
            .runs
            .iter()
            .map(|r| {
                at += r.len;
                at
            })
            .collect();
        let mut cuts = Vec::new();
        for (range, _) in &p.letter_spans {
            if p.text[range.clone()].chars().nth(1).is_some()
                || p.spacers.binary_search(&range.start).is_ok()
            {
                cuts.push(range.start);
            }
            cuts.push(range.end);
        }
        cuts.extend(p.shift_spans.iter().flat_map(|s| [s.0.start, s.0.end]));
        cuts.extend(p.rel_spans.iter().flat_map(|s| [s.0.start, s.0.end]));
        cuts.extend(p.edge_spans.iter().flat_map(|s| [s.0.start, s.0.end]));
        cuts.sort_unstable();
        cuts.dedup();
        let mut box_edges: Vec<_> = p
            .spacers
            .iter()
            .flat_map(|p| [*p, *p + crate::text::inline::SPACER.len()])
            .chain(p.box_extents.iter().flat_map(|b| [b.1, b.2]))
            .collect();
        box_edges.sort_unstable();
        box_edges.dedup();
        Self {
            opportunities: justification::Opportunities::new(p),
            run_edges: Self::run_edges(p, &run_ends),
            box_index: boxes::Boxes::new(p),
            has_bands: p.runs.iter().any(|r| r.background_color.is_some()),
            has_metric_overrides: p
                .runs
                .iter()
                .any(|r| crate::text::fonts::metric_overrides::has_overrides(&r.font)),
            run_ends,
            cuts,
            box_edges,
            relatives: Intervals::new(p.rel_spans.iter().map(|s| s.0.clone())),
            letters: Intervals::new(p.letter_spans.iter().map(|s| s.0.clone())),
            words: Intervals::new(p.word_spans.iter().map(|s| s.0.clone())),
            shifts: Intervals::new(p.shift_spans.iter().map(|s| s.0.clone())),
            boxes: Intervals::new(p.box_spans.iter().map(|s| s.0.clone())),
            edges: Intervals::new(p.edge_spans.iter().map(|s| s.0.clone())),
            decorations: Intervals::new(p.decor_spans.iter().map(|s| s.range.clone())),
            emphases: Intervals::new(p.emph_spans.iter().map(|s| s.range.clone())),
            line_heights: Intervals::new(p.lh_spans.iter().map(|s| s.0.clone())),
            atoms: Intervals::new(p.atom_boxes.iter().map(|b| b.at..b.at + 1)),
            measure_key: OnceCell::new(),
            split_key: OnceCell::new(),
            joined: OnceCell::new(),
            bidi: OnceCell::new(),
            rtl_bidi: OnceCell::new(),
            neutral_bidi: OnceCell::new(),
        }
    }
    fn run_edges(p: &Paragraph, ends: &[usize]) -> Vec<Option<(usize, usize)>> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        let mut edges: Vec<_> = p.edge_spans.iter().map(|s| &s.0).collect();
        edges.sort_unstable_by_key(|r| r.start);
        let mut by_end = BinaryHeap::new();
        let mut by_start = BinaryHeap::new();
        let mut next = 0;
        ends.iter()
            .zip(&p.runs)
            .map(|(&end, run)| {
                let start = end - run.len;
                while let Some(r) = edges.get(next).filter(|r| r.start <= start) {
                    by_end.push(Reverse((r.end, r.start)));
                    by_start.push((r.start, r.end));
                    next += 1;
                }
                while by_end
                    .peek()
                    .is_some_and(|r: &Reverse<(usize, usize)>| r.0.0 < end)
                {
                    by_end.pop();
                }
                while by_start.peek().is_some_and(|r: &(usize, usize)| r.1 < end) {
                    by_start.pop();
                }
                by_start
                    .peek()
                    .zip(by_end.peek())
                    .map(|(a, b)| (a.0, b.0.0))
            })
            .collect()
    }
}

impl Paragraph {
    pub(super) fn prepared(&self) -> &Prepared {
        self.prepared.get_or_init(|| Prepared::new(self))
    }

    pub(super) fn invalidate_prepared(&mut self) {
        self.prepared.take();
    }

    pub(super) fn run_indices(&self, range: &Range<usize>) -> Range<usize> {
        let ends = &self.prepared().run_ends;
        let start = ends.partition_point(|&end| end <= range.start);
        let end = ends
            .partition_point(|&end| end < range.end)
            .saturating_add(1)
            .min(ends.len());
        start..end.max(start)
    }

    pub(super) fn resolved_rtl_visual(&self, range: &Range<usize>) -> Option<bidi::VisualRuns> {
        self.prepared()
            .rtl_bidi
            .get_or_init(|| ResolvedBidi::new(&self.text, Some(Level::rtl())))
            .visual(&self.text, range)
    }

    pub(super) fn resolved_visual(
        &self,
        range: &Range<usize>,
        neutral: bool,
    ) -> Option<bidi::VisualRuns> {
        let forced = if self.plaintext.is_some() && (!neutral || !self.wrap.rtl) {
            None
        } else {
            Some(if self.wrap.rtl {
                Level::rtl()
            } else {
                Level::ltr()
            })
        };
        let prepared = self.prepared();
        if neutral {
            let (text, bidi) = prepared.neutral_bidi.get_or_init(|| {
                let mut text = self.text.to_string();
                for &at in &self.spacers {
                    if text.get(at..at + 3) == Some("\u{feff}") {
                        text.replace_range(at..at + 3, "\u{fffc}");
                    }
                }
                let bidi = ResolvedBidi::new(&text, forced);
                (text, bidi)
            });
            bidi.visual(text, range)
        } else {
            prepared
                .bidi
                .get_or_init(|| ResolvedBidi::new(&self.text, forced))
                .visual(&self.text, range)
        }
    }
}
