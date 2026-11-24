//! The check of a frame's replays and the end of its records.

use gpui::{Bounds, ScaledPixels, Window};
use std::rc::Rc;

impl super::super::FlowReuse {
    /// The check of the frame's replays (`ReplaySlot::refusal`); failed blocks (all of them,
    /// if layout needed a measure) are banned while the flow key is the same.
    pub(in crate::layout::reuse) fn validate_replays(&self, window: &mut Window) -> bool {
        let slots = self.replay_slots.take();
        let refusals: Vec<(u64, bool, &'static str)> = slots
            .iter()
            .filter_map(|slot| {
                let content = slot.record.key.is_none();
                slot.refusal(window).map(|why| (slot.node(), content, why))
            })
            .collect();
        let failed: Vec<u64> = refusals.iter().map(|(node, ..)| *node).collect();
        if !failed.is_empty() {
            // A stale layout does not say which block needed a measurement.
            let all = window.layout_stale();
            crate::why::note("layout_retry.replay_refused", || {
                let contents = refusals.iter().filter(|(_, c, _)| *c).count();
                let mut reasons: Vec<&str> = refusals.iter().map(|(.., why)| *why).collect();
                reasons.sort_unstable();
                reasons.dedup();
                format!(
                    "{} of {} replays refused ({contents} contents), layout stale {all}: {}",
                    failed.len(),
                    slots.len(),
                    reasons.join(",")
                )
            });
            let mut banned = self.store.banned.take();
            banned.extend(
                slots
                    .iter()
                    .map(|slot| slot.node())
                    .filter(|n| all || failed.contains(n)),
            );
            self.store.banned.replace(banned);
            return false;
        }
        self.replay_slots.set(slots);
        true
    }

    /// The replay records of this frame: replayed and finished near screen `screen`.
    pub(in crate::layout::reuse) fn finish_replays(&self, screen: Bounds<ScaledPixels>) {
        // A checked tree may still be discarded if its host moves before paint.
        let slots = self.replay_slots.take();
        if !slots.is_empty() {
            let mut stats = self.store.stats.get();
            let contents = slots.iter().filter(|s| s.record.key.is_none()).count() as u64;
            let blocks = slots.len() as u64 - contents;
            stats.replayed += blocks;
            stats.subtrees += blocks;
            stats.contents += contents;
            self.store.stats.set(stats);
        }
        let mut replays = self.next_replays.take();
        let mut reads = self.store.reads_widths.take();
        let pending = self.pending_replays.take();
        let mut stats = self.store.stats.get();
        stats.replay_tracked += pending.len() as u64;
        self.store.stats.set(stats);
        for (pending, source) in pending {
            let node = source.id();
            if pending.widths.get() {
                reads.insert(node);
            } else {
                reads.remove(&node);
            }
            if let Some(record) = pending.finish(source, Some(screen)) {
                replays.insert(node, Rc::new(record));
            }
        }
        self.store.reads_widths.replace(reads);
        self.store.replays.replace(replays);
    }
}
