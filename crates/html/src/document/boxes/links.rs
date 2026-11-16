//! Чтение связей и эпох коробок для раскладки и слот её результата, без выделений.

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
    /// Эпохи входов раскладки коробки.
    pub(crate) fn epochs(&self, id: BoxId) -> Option<Epochs> {
        self.get(id).map(|r| r.epochs)
    }

    /// Номер последней публикации; эпохи коробок не больше него. Номер меньше
    /// сохранённого потребителем значит, что счёт начался заново (`tick`):
    /// изменено всё.
    pub(crate) fn clock(&self) -> u32 {
        self.clock
    }

    /// Предки коробки от родителя к корню.
    pub(crate) fn ancestors(&self, id: BoxId) -> impl Iterator<Item = BoxId> + '_ {
        let first = self.get(id).map_or(BoxId::NONE, |r| r.parent);
        std::iter::successors(Some(first).filter(|p| self.get(*p).is_some()), |&p| {
            self.get(p)
                .map(|r| r.parent)
                .filter(|q| self.get(*q).is_some())
        })
    }

    /// Дети коробки в порядке документа.
    pub(crate) fn children(&self, id: BoxId) -> impl Iterator<Item = BoxId> + '_ {
        let first = self.get(id).map_or(BoxId::NONE, |r| r.first);
        std::iter::successors(Some(first).filter(|c| self.get(*c).is_some()), |&c| {
            self.get(c)
                .map(|r| r.next)
                .filter(|n| self.get(*n).is_some())
        })
    }

    /// Слот результата раскладки, если раскладка его заводила.
    pub(crate) fn slot(&self, id: BoxId) -> Option<&LayoutSlot> {
        self.get(id)?.layout_slot.as_deref()
    }

    /// Слот результата раскладки (создаётся при первом обращении). Живёт, пока
    /// жива коробка; смена эпох его не сбрасывает — годность решает раскладка.
    pub(crate) fn slot_mut(&mut self, id: BoxId) -> Option<&mut LayoutSlot> {
        let record = self.arena.record_mut(id)?;
        Some(record.layout_slot.get_or_insert_with(Default::default))
    }
}

impl BoxTree {
    /// Эпоха сборки (`Epochs::paint`) коробки `id`: номер
    /// последней публикации, изменившей что-либо, что читает сборка её
    /// поддерева. `None` — у источника нет коробки.
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

    /// Следующий номер публикации. При переполнении все коробки получают
    /// эпохи 1 и номер 1: потребитель с прежним номером видит их изменёнными.
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

    /// Поставить эпохи изменённым коробкам и их предкам: `child` — до корня,
    /// `intrinsic` — до первой коробки с внешним вкладом. Подъём останавливается
    /// на предке, уже получившем эту эпоху: O(изменённых + глубина).
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
            // Вклад меняется у самой коробки; дальше — если он не внешний.
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

    /// `paint` коробки и её предков до первого, уже получившего эту эпоху.
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

    /// Убрать коробку со всем поддеревом; ключи их слотов больше не разрешаются.
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
