//! Reading box links and epochs for layout, and its result slot, without allocation.

use super::BoxTree;
use super::epochs::{Change, Epochs};
use crate::dom::BoxId;
use crate::layout::retained::LayoutSlot;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the per-box layout cache lands before its consumers"
    )
)]
impl BoxTree {
    /// Epochs of the layout inputs of a box.
    pub(crate) fn epochs(&self, id: BoxId) -> Option<Epochs> {
        self.get(id).map(|r| r.epochs)
    }

    /// The number of the last publication; box epochs are not above it. A number below
    /// the one a consumer kept means the count started over (`tick`):
    /// everything changed.
    pub(crate) fn clock(&self) -> u32 {
        self.clock
    }

    /// Ancestors of the box from the parent to the root.
    pub(crate) fn ancestors(&self, id: BoxId) -> impl Iterator<Item = BoxId> + '_ {
        let first = self.get(id).map_or(BoxId::NONE, |r| r.parent);
        std::iter::successors(Some(first).filter(|p| self.get(*p).is_some()), |&p| {
            self.get(p)
                .map(|r| r.parent)
                .filter(|q| self.get(*q).is_some())
        })
    }

    /// Children of the box in document order.
    pub(crate) fn children(&self, id: BoxId) -> impl Iterator<Item = BoxId> + '_ {
        let first = self.get(id).map_or(BoxId::NONE, |r| r.first);
        std::iter::successors(Some(first).filter(|c| self.get(*c).is_some()), |&c| {
            self.get(c)
                .map(|r| r.next)
                .filter(|n| self.get(*n).is_some())
        })
    }

    /// The layout result slot, if layout created it.
    pub(crate) fn slot(&self, id: BoxId) -> Option<&LayoutSlot> {
        self.get(id)?.layout_slot.as_deref()
    }

    /// The layout result slot (created on first access). It lives as long
    /// as the box; an epoch change does not reset it: layout decides its validity.
    pub(crate) fn slot_mut(&mut self, id: BoxId) -> Option<&mut LayoutSlot> {
        let record = self.arena.record_mut(id)?;
        Some(record.layout_slot.get_or_insert_with(Default::default))
    }
}

impl BoxTree {
    /// The build epoch (`Epochs::paint`) of box `id`: the number
    /// of the last publication that changed anything the build of its
    /// subtree reads. `None`: the source has no box.
    pub(crate) fn build_epoch(&self, id: BoxId) -> Option<u32> {
        Some(self.get(id)?.epochs.paint)
    }

    /// The one box whose children the boxes of `nodes` are: the parent of
    /// the box of every keyed element of `nodes`. `None` when they have
    /// no keyed element, or their boxes have different parents (an
    /// anonymous wrapper holds part of them), so no box answers for all.
    pub(crate) fn owner_of(&self, nodes: &[crate::dom::Node]) -> Option<BoxId> {
        let mut owner = None;
        for node in nodes {
            let crate::dom::Node::Element(e) = node else {
                continue;
            };
            let parent = self.get(self.box_of(e.node_id)?)?.parent;
            if *owner.get_or_insert(parent) != parent {
                return None;
            }
        }
        owner.filter(|id: &BoxId| self.get(*id).is_some())
    }

    /// The next publication number. On overflow every box gets
    /// epochs 1 and the number 1: a consumer with an older number sees them changed.
    pub(super) fn tick(&mut self) -> u32 {
        if self.clock == u32::MAX {
            let ids: Vec<_> = self.iter().map(|(id, _)| id).collect();
            for id in ids {
                if let Some(r) = self.arena.record_mut(id) {
                    r.epochs = Epochs {
                        layout: 1,
                        child: 1,
                        intrinsic: 1,
                        paint: 1,
                    };
                }
            }
            self.clock = 1;
        }
        self.clock += 1;
        self.clock
    }

    /// Set epochs on the changed boxes and their ancestors: `child` up to the root,
    /// `intrinsic` up to the first box with an external contribution. The walk stops
    /// at an ancestor that already has this epoch: O(changed + depth).
    pub(super) fn propagate(&mut self, clock: u32, changed: &[(BoxId, Change)]) {
        for &(id, change) in changed {
            let Some(record) = self.arena.record_mut(id) else {
                continue;
            };
            if change == Change::Paint {
                self.mark_paint(id, clock);
                continue;
            }
            record.epochs.layout = clock;
            // The contribution changes for the box itself; further up unless it is external.
            let own = change == Change::Own;
            let mut stop = !own && record.external;
            record.epochs.intrinsic = clock;
            let (mut child, mut at) = (true, record.parent);
            while child || !stop {
                let Some(parent) = self.arena.record_mut(at) else {
                    break;
                };
                if child {
                    child = parent.epochs.child != clock;
                    parent.epochs.child = clock;
                }
                if !stop {
                    stop = parent.epochs.intrinsic == clock || parent.external;
                    parent.epochs.intrinsic = clock;
                }
                at = parent.parent;
            }
            self.mark_paint(id, clock);
        }
    }

    /// Whether the size of box `id` may follow its siblings' content
    /// changed after publication `at`: an ancestor that sizes its
    /// children from their intrinsic contributions (`BoxRecord::distributes`)
    /// got a newer intrinsic epoch.
    pub(crate) fn resized_by_siblings(&self, id: BoxId, at: u32) -> bool {
        let mut parent = self.get(id).map_or(BoxId::NONE, |r| r.parent);
        while let Some(record) = self.get(parent) {
            if record.distributes && record.epochs.intrinsic > at {
                return true;
            }
            parent = record.parent;
        }
        false
    }

    /// `paint` of the box and its ancestors up to the first that already has this epoch.
    fn mark_paint(&mut self, id: BoxId, clock: u32) {
        let mut at = id;
        while let Some(record) = self.arena.record_mut(at) {
            if record.epochs.paint == clock {
                break;
            }
            record.epochs.paint = clock;
            at = record.parent;
        }
    }

    /// Remove a box with its whole subtree; the keys of their slots no longer resolve.
    pub(super) fn remove_subtree(&mut self, id: BoxId) {
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            let Some(record) = self.arena.remove(id) else {
                continue;
            };
            self.flows.forget(id);
            if record.source != 0 {
                self.by_node.remove_if(record.source, id);
            }
            let mut child = record.first;
            while let Some(next) = self.get(child).map(|r| r.next) {
                pending.push(child);
                child = next;
            }
        }
    }
}
