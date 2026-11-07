//! Выделение может разрезать два прогона: его собственные границы готовятся
//! один раз на отрисовку, затем каждый bidi-кусок получает только свои стили.
//! Без выделения и без собственного оформления абзаца прогоны рисуются как
//! есть: их и концы берём у абзаца взаймы, не копируя на каждую отрисовку.

use gpui::TextRun;
use std::{borrow::Cow, ops::Range};

pub(crate) struct PaintRuns<'a> {
    runs: Cow<'a, [TextRun]>,
    ends: Cow<'a, [usize]>,
}

impl<'a> PaintRuns<'a> {
    pub(in crate::text::paragraph) fn new(runs: Vec<TextRun>) -> Self {
        let mut at = 0;
        let ends = runs
            .iter()
            .map(|r| {
                at += r.len;
                at
            })
            .collect::<Vec<_>>();
        Self {
            runs: Cow::Owned(runs),
            ends: Cow::Owned(ends),
        }
    }

    /// The paragraph's own runs with their prefix ends (`Prepared::run_ends`).
    pub(in crate::text::paragraph) fn borrowed(runs: &'a [TextRun], ends: &'a [usize]) -> Self {
        debug_assert_eq!(runs.len(), ends.len());
        Self {
            runs: Cow::Borrowed(runs),
            ends: Cow::Borrowed(ends),
        }
    }

    pub(in crate::text::paragraph) fn slice(&self, range: &Range<usize>) -> Vec<TextRun> {
        let start = self.ends.partition_point(|&end| end <= range.start);
        let end = self
            .ends
            .partition_point(|&end| end < range.end)
            .saturating_add(1)
            .min(self.runs.len());
        let mut out = Vec::with_capacity(end.saturating_sub(start));
        out.extend((start..end).filter_map(|i| {
            let len = self.ends[i]
                .min(range.end)
                .saturating_sub((self.ends[i] - self.runs[i].len).max(range.start));
            (len > 0).then(|| {
                let mut run = self.runs[i].clone();
                run.len = len;
                run
            })
        }));
        out
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
        let ends: Vec<usize> = runs
            .iter()
            .scan(0, |at, r| {
                *at += r.len;
                Some(*at)
            })
            .collect();
        let borrowed = PaintRuns::borrowed(&runs, &ends);
        for start in 0..=8 {
            for end in start..=8 {
                let lens = |runs: Vec<TextRun>| runs.into_iter().map(|r| r.len).collect::<Vec<_>>();
                let expected = lens(crate::text::paragraph::slice_runs(&runs, &(start..end)));
                assert_eq!(lens(prepared.slice(&(start..end))), expected);
                assert_eq!(lens(borrowed.slice(&(start..end))), expected);
            }
        }
    }
}
