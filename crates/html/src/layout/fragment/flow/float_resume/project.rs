//! Проекция незавершённого float независимо от остатка обычного потока.
use super::{Cursor, Snapshot};
use crate::layout::fragment::flow::inline::Inline;

impl Cursor {
    pub(crate) fn offset(&self, fresh: &Snapshot, paragraphs: &[Inline]) -> Option<f32> {
        if self.token.at_block_end {
            // Конец border box не погашает оставшееся нижнее поле float
            // (css-break-3 §5.2): clear продолжает отсчитываться от margin box.
            let margin_consumed = self.boundary - self.token.consumed;
            return (margin_consumed < fresh.bounds.margins[1].max(0.0))
                .then_some(fresh.bounds.bottom + margin_consumed);
        }
        if self.boundary < 0.0 {
            return Some(fresh.bounds.top + self.boundary);
        }
        let mapped = self.mapped(paragraphs, &fresh.scope);
        let height = (fresh.bounds.bottom - fresh.bounds.top).max(0.0);
        Some(mapped.unwrap_or(fresh.bounds.top + self.boundary.min(height)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{paragraph, snapshot};
    use super::*;

    #[test]
    fn monolithic_float_survives_exhausted_normal_flow() {
        let old = snapshot(&[2, 2], 115.2).translated(192.);
        let cursor = Cursor::at(&old, &[], 192.);
        assert_eq!(cursor.offset(&old, &[]), Some(192.));
        let done = snapshot(&[2, 1], 115.2).translated(18.);
        assert_eq!(Cursor::at(&done, &[], 192.).offset(&done, &[]), None);
    }

    #[test]
    fn future_float_keeps_gap_before_its_start() {
        let old = snapshot(&[1], 80.).translated(100.);
        let fresh = old.translated(50.);
        assert_eq!(Cursor::at(&old, &[], 60.).offset(&fresh, &[]), Some(110.));
        let cursor = Cursor::at(&old, &[paragraph(&[1], 10.).translated(100.)], 60.);
        let paragraphs = [paragraph(&[1], 20.).translated(150.)];
        assert_eq!(cursor.offset(&fresh, &paragraphs), Some(110.));
    }

    #[test]
    fn finished_border_keeps_unconsumed_bottom_margin() {
        let mut old = snapshot(&[1], 48.);
        old.bounds.margins[1] = 96.;
        let fresh = old.translated(20.);
        for from in [48., 80., 143.] {
            assert_eq!(
                Cursor::at(&old, &[], from).offset(&fresh, &[]),
                Some(from + 20.)
            );
        }
        for from in [144., 160.] {
            assert_eq!(Cursor::at(&old, &[], from).offset(&fresh, &[]), None);
        }
        old.bounds.margins[1] = -20.;
        assert_eq!(Cursor::at(&old, &[], 48.).offset(&old, &[]), None);
    }

    #[test]
    fn float_projects_its_own_scoped_line_after_reflow() {
        let old = snapshot(&[1, 2], 80.);
        let cursor = Cursor::at(&old, &[paragraph(&[1, 2], 10.)], 30.);
        let fresh = snapshot(&[1, 2], 160.).translated(100.);
        let paragraphs = [
            paragraph(&[1, 3], 5.),
            paragraph(&[1, 2], 20.).translated(100.),
        ];
        assert_eq!(cursor.offset(&fresh, &paragraphs), Some(160.));
    }
}
