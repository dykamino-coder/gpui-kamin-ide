//! Proof that cached ancestor replays kept a host record in the same slots.

use super::*;
use std::cell::Cell;

/// A host record's continuity through frames that reuse its cached ancestor.
/// This proof deliberately expires when any slot moves; it never rebases ranges.
#[derive(Clone)]
pub struct RetainedContinuity(Rc<Continuity>);

pub(super) struct Continuity {
    window: AnyWindowHandle,
    prepaint: RetainedPrepaint,
    paint: RetainedPaint,
    prepaint_serial: Cell<u64>,
    paint_serial: Cell<u64>,
}

impl RetainedContinuity {
    /// The record is still in the rendered frame of this window.
    pub fn is_previous(&self, window: &Window) -> bool {
        self.0.window == window.window_handle()
            && self.0.prepaint_serial.get().checked_add(1) == Some(window.frame_serial())
            && self.0.paint_serial.get() == self.0.prepaint_serial.get()
    }
}

fn contains<const N: usize>(start: [u32; N], end: [u32; N], a: [u32; N], b: [u32; N]) -> bool {
    (0..N).all(|i| start[i] <= a[i] && b[i] <= end[i])
}

fn advance(serial: &Cell<u64>, now: u64) {
    if serial.get().checked_add(1) == Some(now) {
        serial.set(now);
    }
}

impl Window {
    /// Track a completed record without owning it beyond the host's lifetime.
    pub fn retain_continuity(
        &mut self,
        prepaint: &RetainedPrepaint,
        paint: &RetainedPaint,
    ) -> RetainedContinuity {
        self.retained_continuity
            .retain(|record| record.strong_count() != 0);
        let record = Rc::new(Continuity {
            window: self.window_handle(),
            prepaint: prepaint.clone(),
            paint: paint.clone(),
            prepaint_serial: Cell::new(self.frame_serial()),
            paint_serial: Cell::new(self.frame_serial()),
        });
        self.retained_continuity.push(Rc::downgrade(&record));
        RetainedContinuity(record)
    }

    pub(super) fn carry_retained_prepaint(&self, range: &Range<PrepaintStateIndex>) {
        let (start, end) = (range.start.pack(), range.end.pack());
        for record in self
            .retained_continuity
            .iter()
            .filter_map(std::rc::Weak::upgrade)
        {
            let recorded = record.prepaint.range();
            let (a, b) = (recorded.start.pack(), recorded.end.pack());
            if contains(start.0, end.0, a.0, b.0) && contains(start.1, end.1, a.1, b.1) {
                advance(&record.prepaint_serial, self.frame_serial());
            }
        }
    }

    pub(super) fn paint_slots_match(&self, start: &PaintIndex) -> bool {
        let (start, now) = (start.pack(), self.paint_index().pack());
        let same = start.words == now.words;
        #[cfg(any(test, feature = "test-support"))]
        let same = same && start.debug_bounds_index == now.debug_bounds_index;
        same
    }

    pub(super) fn carry_retained_paint(&self, range: &Range<PaintIndex>) {
        let (start, end) = (range.start.pack(), range.end.pack());
        // A scene-layer source has a different index space. Group relabeling
        // likewise makes an old record's group indices unusable.
        if self.replay_source().is_some() || self.next_frame.scene.relabel != 0 {
            return;
        }
        for record in self
            .retained_continuity
            .iter()
            .filter_map(std::rc::Weak::upgrade)
        {
            let recorded = record.paint.range();
            let (a, b) = (recorded.start.pack(), recorded.end.pack());
            let same = contains(start.words, end.words, a.words, b.words)
                && contains(start.line, end.line, a.line, b.line);
            #[cfg(any(test, feature = "test-support"))]
            let same = same
                && start.debug_bounds_index <= a.debug_bounds_index
                && b.debug_bounds_index <= end.debug_bounds_index;
            if same && record.paint.groups().is_empty() {
                advance(&record.paint_serial, self.frame_serial());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_frame_expires_continuity_permanently() {
        let serial = Cell::new(7);
        advance(&serial, 8);
        advance(&serial, 10);
        advance(&serial, 11);
        assert_eq!(serial.get(), 8);
    }

    #[test]
    fn a_record_must_be_contained_in_every_index_space() {
        assert!(contains([1, 3], [9, 7], [2, 4], [9, 5]));
        assert!(!contains([1, 3], [9, 7], [2, 2], [9, 5]));
        assert!(!contains([1, 3], [9, 7], [2, 4], [9, 8]));
    }
}
