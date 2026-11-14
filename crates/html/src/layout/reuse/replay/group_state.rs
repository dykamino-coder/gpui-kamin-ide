//! Состояние повтора групп (`super::group`): записи между кадрами, места и
//! незавершённые записи одного кадра, проверка кадра и её последствия.

use super::group::{GroupRecord, GroupSlot, PendingGroup};
use gpui::Window;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Group records kept between frames (`Store`).
#[derive(Default)]
pub(in crate::layout::reuse) struct GroupStore {
    pub(in crate::layout::reuse) records: RefCell<HashMap<u64, Rc<GroupRecord>>>,
    /// The flow key of the last build with groups, shared by their records.
    pub(in crate::layout::reuse) flow: RefCell<Option<Rc<crate::layout::reuse::FlowKey>>>,
    /// Frames in a row whose group replays failed the frame check.
    strikes: Cell<u8>,
    /// Frames left without group replays after repeated failures.
    cooldown: Cell<u32>,
}

impl GroupStore {
    pub(in crate::layout::reuse) fn reset(&self) {
        self.records.take();
        self.flow.take();
        self.strikes.set(0);
        self.cooldown.set(0);
    }

    /// Group replays are tried in this frame.
    pub(in crate::layout::reuse) fn open(&self) -> bool {
        self.cooldown.get() == 0
    }
}

/// Group replays of one build (`FlowReuse`).
#[derive(Default)]
pub(in crate::layout::reuse) struct Groups {
    pub(in crate::layout::reuse) slots: Cell<Vec<Rc<GroupSlot>>>,
    pub(in crate::layout::reuse) pending: RefCell<Vec<Rc<PendingGroup>>>,
    pub(in crate::layout::reuse) next: RefCell<HashMap<u64, Rc<GroupRecord>>>,
}

impl crate::layout::reuse::FlowReuse {
    /// The frame check of the group replays (`GroupSlot::valid`). A failed
    /// frame is built again without replays, and its build records every
    /// group at its new place, so a refused group is not banned: the next
    /// frame replays it again. Only failures frame after frame (a table that
    /// moves or resizes on every frame) pause the replays, for a number of
    /// frames that doubles with each failure, so such frames stop paying for
    /// two builds while a table that settles is replayed again soon.
    pub(in crate::layout::reuse) fn validate_groups(&self, window: &mut Window) -> bool {
        let slots = self.groups.slots.take();
        if slots.is_empty() {
            return true;
        }
        let store = &self.store.groups;
        let ok = slots.iter().all(|slot| slot.valid(window));
        if ok {
            store.strikes.set(0);
            let mut stats = self.store.stats.get();
            stats.groups += slots.len() as u64;
            self.store.stats.set(stats);
        } else {
            let strikes = store.strikes.get().saturating_add(1).min(6);
            store.strikes.set(strikes);
            if strikes >= 2 {
                store.cooldown.set(1 << strikes);
            }
        }
        ok
    }

    /// The group records of this frame: the replayed and the finished ones.
    pub(in crate::layout::reuse) fn finish_groups(&self) {
        let store = &self.store.groups;
        store.cooldown.set(store.cooldown.get().saturating_sub(1));
        let mut next = self.groups.next.take();
        for pending in self.groups.pending.take() {
            if let Some((node, record)) = pending.finish() {
                next.insert(node, Rc::new(record));
            }
        }
        store.records.replace(next);
    }
}