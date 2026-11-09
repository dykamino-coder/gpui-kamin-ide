//! The far state of one build and its hand-over to the document (`Store`).

use super::{FarRecord, FarSlot, PendingFar};
use crate::layout::reuse::{FlowReuse, Source};
use std::collections::HashMap;
use std::rc::Rc;

/// Far blocks of one build (`FlowReuse::far`), allocated with the first one:
/// records claimed, recordings pending and slots to check before prepaint.
#[derive(Default)]
pub(crate) struct FrameFar {
    pub(in crate::layout::reuse) next: HashMap<u64, Rc<FarRecord>>,
    pub(in crate::layout::reuse) pending: Vec<(Rc<PendingFar>, Source)>,
    pub(in crate::layout::reuse) slots: Vec<Rc<FarSlot>>,
}

impl FlowReuse {
    /// This build's far state, made with its first use.
    pub(in crate::layout::reuse) fn with_far<R>(&self, f: impl FnOnce(&mut FrameFar) -> R) -> R {
        let mut far = self.far.take().unwrap_or_default();
        let out = f(&mut far);
        self.far.set(Some(far));
        out
    }

    /// After paint: far records claimed in this frame and those recorded.
    /// A failed recording marks its block near (`Store::far_near`).
    pub(in crate::layout::reuse) fn finish_far(&self) {
        let Some(frame) = self.far.take() else {
            // No far block in this build: no record is carried over.
            self.store.far.take();
            return;
        };
        let claimed = frame.next.len() as u64;
        let mut records = frame.next;
        for (pending, source) in frame.pending {
            let node = source.id();
            match pending.finish(source) {
                Some(record) => {
                    self.store.forget_near(node);
                    records.insert(node, Rc::new(record));
                }
                None => self.store.note_near(node),
            }
        }
        // The kept map reuses its box between frames.
        let kept = (!records.is_empty()).then(|| {
            let mut kept = self.store.far.take().unwrap_or_default();
            *kept = records;
            kept
        });
        self.store.far.set(kept);
        let mut stats = self.store.stats.get();
        stats.far += claimed;
        self.store.stats.set(stats);
    }
}
