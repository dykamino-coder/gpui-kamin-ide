//! Tracking the outside width reads of one build step in a shared context
//! (`BoxConstraints::track_scoped`): the step records into its own flag, the
//! subtree tracked around it still sees its reads, and the context is as it
//! was once the step ends.

use super::{BothSinks, BoxConstraints, WidthReads, WidthSink};
use std::cell::Cell;
use std::rc::Rc;

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
}
