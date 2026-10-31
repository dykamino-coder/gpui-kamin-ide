//! Доступ к прогонам по байтовому индексу; нарезка полосы сохраняет по одному
//! соседу для прежних правил продолжения рамки через границу слова.

use super::super::*;
use gpui::TextRun;
use std::ops::Range;

impl Paragraph {
    pub(in crate::text::paragraph) fn run_at(&self, at: usize) -> Option<usize> {
        let i = self.prepared().run_ends.partition_point(|&end| end <= at);
        (i < self.runs.len()).then_some(i)
    }

    pub(in crate::text::paragraph) fn own_runs(&self, range: &Range<usize>) -> Vec<TextRun> {
        self.run_indices(range)
            .filter_map(|i| {
                let end = self.prepared().run_ends[i];
                let start = end - self.runs[i].len;
                let len = end.min(range.end).saturating_sub(start.max(range.start));
                (len > 0).then(|| {
                    let mut run = self.runs[i].clone();
                    run.len = len;
                    run
                })
            })
            .collect()
    }

    pub(in crate::text::paragraph) fn own_banded_runs(&self, range: &Range<usize>) -> Vec<TextRun> {
        let start = range
            .start
            .checked_sub(1)
            .and_then(|at| self.run_at(at))
            .unwrap_or(0);
        let end = self.run_at(range.end).map_or(self.runs.len(), |i| i + 1);
        let offset = start
            .checked_sub(1)
            .map_or(0, |i| self.prepared().run_ends[i]);
        slice_runs_banded(
            &self.runs[start..end],
            &(range.start - offset..range.end - offset),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn indexed_slices_keep_lengths_and_zero_length_run_boundaries() {
        let p = Paragraph::new(
            "abcdefgh".into(),
            [0, 2, 0, 0, 0, 3, 3, 0]
                .into_iter()
                .map(|len| TextRun {
                    len,
                    background_color: Some(gpui::Hsla::default()),
                    background_pad: [px(3.); 4],
                    background_border: Some((gpui::Hsla::default(), [px(2.); 4])),
                    ..Default::default()
                })
                .collect(),
            px(16.),
            px(20.),
            Align::Left,
            Wrap::default(),
        );
        for start in 0..=8 {
            for end in start..=8 {
                let expected: Vec<_> = slice_runs(&p.runs, &(start..end))
                    .iter()
                    .map(|r| r.len)
                    .collect();
                assert_eq!(
                    p.own_runs(&(start..end))
                        .iter()
                        .map(|r| r.len)
                        .collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(
                    p.own_banded_runs(&(start..end)),
                    slice_runs_banded(&p.runs, &(start..end))
                );
            }
        }
    }
}
