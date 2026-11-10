//! The far state of one build and its hand-over to the document (`Store`).

use super::{FarSlot, PendingFar};
use crate::layout::reuse::{FlowReuse, Source};
use gpui::InkSink;
use std::rc::Rc;

/// Far blocks of one build (`FlowReuse::far`), allocated with the first one:
/// the count of records claimed, recordings pending and slots to check
/// before prepaint.
#[derive(Default)]
pub(crate) struct FrameFar {
    pub(in crate::layout::reuse) claimed: u64,
    /// By value: a recorder reaches its own by index (`FarRecorder`).
    pub(in crate::layout::reuse) pending: Vec<(PendingFar, Source)>,
    pub(in crate::layout::reuse) slots: Vec<Rc<FarSlot>>,
    /// Outside width reads of the far candidate being built, shared by all
    /// candidates of the build (`Mark::shared_widths`).
    pub(in crate::layout::reuse) widths: Rc<std::cell::Cell<bool>>,
    /// The flow's options tracking `widths`, made for the first candidate
    /// and lent to each one's build (`FlowCx::child_opts`).
    pub(in crate::layout::reuse) opts: Option<Box<crate::layout::options::RenderOpts>>,
    /// The ink sink of the recorders' paints, one scope per recorder
    /// (`InkSink::begin_scope`).
    pub(in crate::layout::reuse) sink: Rc<InkSink>,
}

impl FlowReuse {
    /// This build's far state, made with its first use.
    pub(in crate::layout::reuse) fn with_far<R>(&self, f: impl FnOnce(&mut FrameFar) -> R) -> R {
        let mut far = self.far.take().unwrap_or_default();
        let out = f(&mut far);
        self.far.set(Some(far));
        out
    }

    /// The pending recording `index` of this build, while the build is not
    /// finished (a later paint of the same tree finds none).
    pub(in crate::layout::reuse) fn with_pending<R>(
        &self,
        index: usize,
        f: impl FnOnce(&mut PendingFar) -> R,
    ) -> Option<R> {
        let mut far = self.far.take()?;
        let out = far.pending.get_mut(index).map(|(pending, _)| f(pending));
        self.far.set(Some(far));
        out
    }

    /// The recorders' ink sink of this build.
    pub(in crate::layout::reuse) fn far_sink(&self) -> Option<Rc<InkSink>> {
        let far = self.far.take()?;
        let sink = far.sink.clone();
        self.far.set(Some(far));
        Some(sink)
    }

    /// After paint: far records claimed in this frame and those recorded.
    /// A failed recording marks its block near (`Store::far_near`).
    pub(in crate::layout::reuse) fn finish_far(&self) {
        let Some(frame) = self.far.take() else {
            // No far block in this build: no record is carried over.
            self.store.far.take();
            return;
        };
        let claimed = frame.claimed;
        let mut kept = self.store.far.take().unwrap_or_default();
        // The records claimed by this build stay; a build that claimed none
        // (the second build of a refused frame) keeps none, like the marks
        // of a refused build it replaces.
        kept.retain(|_, record| record.claimed.replace(false) && claimed > 0);
        for (pending, source) in frame.pending {
            let node = source.id();
            match pending.finish(source, &frame.sink) {
                Some(record) => {
                    self.store.forget_near(node);
                    kept.insert(node, record);
                }
                None => self.store.note_near(node),
            }
        }
        // The kept map reuses its box and table between frames.
        self.store.far.set((!kept.is_empty()).then_some(kept));
        let mut stats = self.store.stats.get();
        stats.far += claimed;
        self.store.stats.set(stats);
    }
}
