//! Выделение может разрезать два прогона: его собственные границы готовятся
//! один раз на отрисовку, затем каждый bidi-кусок получает только свои стили.

use gpui::TextRun;
use std::ops::Range;

pub(crate) struct PaintRuns {
    runs: Vec<TextRun>,
    ends: Vec<usize>,
}

impl PaintRuns {
    pub(in crate::text::paragraph) fn new(runs: Vec<TextRun>) -> Self {
        let mut at = 0;
        let ends = runs
            .iter()
            .map(|r| {
                at += r.len;
                at
            })
            .collect();
        Self { runs, ends }
    }

    pub(in crate::text::paragraph) fn slice(&self, range: &Range<usize>) -> Vec<TextRun> {
        let start = self.ends.partition_point(|&end| end <= range.start);
        let end = self
            .ends
            .partition_point(|&end| end < range.end)
            .saturating_add(1)
            .min(self.runs.len());
        (start..end)
            .filter_map(|i| {
                let len = self.ends[i]
                    .min(range.end)
                    .saturating_sub((self.ends[i] - self.runs[i].len).max(range.start));
                (len > 0).then(|| {
                    let mut run = self.runs[i].clone();
                    run.len = len;
                    run
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_boundaries_keep_original_run_slices() {
        let runs = [0, 2, 1, 0, 2, 3]
            .into_iter()
            .map(|len| TextRun {
                len,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let prepared = PaintRuns::new(runs.clone());
        for start in 0..=8 {
            for end in start..=8 {
                let lens = |runs: Vec<TextRun>| runs.into_iter().map(|r| r.len).collect::<Vec<_>>();
                assert_eq!(
                    lens(prepared.slice(&(start..end))),
                    lens(crate::text::paragraph::slice_runs(&runs, &(start..end)))
                );
            }
        }
    }
}
