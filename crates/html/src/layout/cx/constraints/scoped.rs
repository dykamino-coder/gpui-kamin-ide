//! Tracking the outside width reads of one build step in a shared context
//! (`BoxConstraints::track_scoped`): the step records into its own flag, the
//! subtree tracked around it still sees its reads, and the context is as it
//! was once the step ends.

use super::{BoxConstraints, WidthReads, WidthSink};
use std::cell::Cell;
use std::rc::Rc;

impl WidthSink for Cell<bool> {
    fn note(&self) {
        self.set(true);
    }
}

/// A nested subtree's sink and its tracked ancestor's (`track_widths_below`).
pub(super) struct BothSinks(pub(super) Rc<dyn WidthSink>, pub(super) Rc<dyn WidthSink>);

impl WidthSink for BothSinks {
    fn note(&self) {
        self.0.note();
        self.1.note();
    }
}

/// The accessors `track_reads` tells apart: `cb_width` and `avail_w`.
pub(crate) const CB_WIDTH: u8 = 1;
pub(crate) const AVAIL_W: u8 = 2;

/// What `track_reads` replaced in the context.
pub(crate) struct ScopedReads(Option<Rc<Cell<u8>>>);

impl BoxConstraints {
    /// Record into `read` every width read of one build step until `untrack_reads`,
    /// outside or not, as a set of accessors (`CB_WIDTH`, `AVAIL_W`), in place: the copies
    /// made meanwhile record too. Unlike `track_scoped`, what the tracked subtrees around
    /// the step record is unchanged.
    pub(crate) fn track_reads(&self, read: Rc<Cell<u8>>) -> ScopedReads {
        ScopedReads(self.widths.any.replace(Some(read)))
    }

    /// End `track_reads`: the context records as before it.
    pub(crate) fn untrack_reads(&self, saved: ScopedReads) {
        self.widths.any.set(saved.0);
    }

    /// The widths the accessors in `reads` give, read through them: a tracked subtree
    /// around the step records them as the step itself did.
    pub(crate) fn read_widths(&self, reads: u8) -> (Option<f32>, Option<f32>) {
        let cb = (reads & CB_WIDTH != 0).then(|| self.cb_width()).flatten();
        let avail = (reads & AVAIL_W != 0).then(|| self.avail_w()).flatten();
        (cb, avail)
    }
}

/// What a scoped tracking replaced in the context (`BoxConstraints::untrack_scoped`).
pub(crate) struct ScopedWidths {
    sink: Option<Rc<dyn WidthSink>>,
    outside: (bool, bool),
}

impl BoxConstraints {
    /// Record into `read` every read of a width from outside this context
    /// until `untrack_scoped`, in place: the copies made meanwhile record too.
    pub(crate) fn track_scoped(&self, read: Rc<Cell<bool>>) -> ScopedWidths {
        let outside = (self.cb_outside.get(), self.avail_outside.get());
        let sink = self.widths.sink();
        let own: Rc<dyn WidthSink> = match sink.clone().filter(|_| outside.0 || outside.1) {
            Some(outer) => Rc::new(BothSinks(read, outer)),
            None => read,
        };
        self.widths.sink.set(Some(own));
        self.cb_outside.set(true);
        self.avail_outside.set(true);
        ScopedWidths { sink, outside }
    }

    /// End `track_scoped`: the context records as before it.
    pub(crate) fn untrack_scoped(&self, saved: ScopedWidths) {
        self.widths.sink.set(saved.sink);
        self.cb_outside.set(saved.outside.0);
        self.avail_outside.set(saved.outside.1);
    }
}

impl WidthReads {
    #[cfg(test)]
    fn tracked(&self) -> bool {
        self.sink().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scoped_read_is_recorded_and_the_context_is_restored() {
        let cx = BoxConstraints::default();
        let read = Rc::new(Cell::new(false));
        let saved = cx.track_scoped(read.clone());
        let copy = cx.clone();
        assert!(!read.get());
        let _ = copy.avail_w();
        assert!(read.get(), "a copy made in the step records its read");
        cx.untrack_scoped(saved);
        assert!(!cx.widths.tracked() && !cx.cb_outside.get() && !cx.avail_outside.get());
    }

    #[test]
    fn a_scoped_read_inside_a_tracked_subtree_reaches_both() {
        let cx = BoxConstraints::default();
        let outer = Rc::new(Cell::new(false));
        cx.track_widths_in_place(outer.clone());
        let read = Rc::new(Cell::new(false));
        let saved = cx.track_scoped(read.clone());
        let _ = cx.cb_width();
        cx.untrack_scoped(saved);
        assert!(read.get() && outer.get());
        let again = Rc::new(Cell::new(false));
        let saved = cx.track_scoped(again.clone());
        cx.untrack_scoped(saved);
        let _ = cx.cb_width();
        assert!(!again.get(), "no read after the step");
    }

    #[test]
    fn a_read_of_a_step_is_recorded_without_changing_the_outer_record() {
        let cx = BoxConstraints::default();
        let outer = Rc::new(Cell::new(false));
        cx.track_widths_in_place(outer.clone());
        let read = Rc::new(Cell::new(0));
        let saved = cx.track_reads(read.clone());
        let copy = cx.clone();
        let _ = copy.cb_width();
        cx.untrack_reads(saved);
        assert_eq!(
            read.get(),
            CB_WIDTH,
            "a copy made in the step records its read"
        );
        assert_eq!(
            outer.get(),
            cx.cb_outside.get(),
            "the outer record sees only outside reads"
        );
    }
}
