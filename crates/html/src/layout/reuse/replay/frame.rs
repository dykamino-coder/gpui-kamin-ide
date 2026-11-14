//! The replays of one frame: their check before prepaint and the records
//! kept for the next frame (`super`).

use crate::layout::reuse::FlowReuse;
use gpui::{Bounds, ScaledPixels, Window};
use std::rc::Rc;

impl FlowReuse {
    /// Проверка повторов кадра (`ReplaySlot::valid`); неудачные блоки (все,
    /// если раскладке понадобилась мера) запрещаются, пока ключ потока тот же.
    pub(in crate::layout::reuse) fn validate_replays(&self, window: &mut Window) -> bool {
        let slots = self.replay_slots.take();
        let failed: Vec<u64> = slots
            .iter()
            .filter(|slot| !slot.valid(window))
            .map(|slot| slot.node())
            .collect();
        if !failed.is_empty() {
            // A stale layout does not say which block needed a measurement.
            let all = window.layout_stale();
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
        if !slots.is_empty() {
            let mut stats = self.store.stats.get();
            stats.replayed += slots.len() as u64;
            stats.subtrees += slots.len() as u64;
            self.store.stats.set(stats);
        }
        true
    }

    /// Записи повтора этого кадра: повторённые и завершённые у экрана `screen`.
    pub(in crate::layout::reuse) fn finish_replays(&self, screen: Bounds<ScaledPixels>) {
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
            if let Some(record) = pending.finish(source, screen) {
                replays.insert(node, Rc::new(record));
            }
        }
        self.store.reads_widths.replace(reads);
        self.store.replays.replace(replays);
    }
}
