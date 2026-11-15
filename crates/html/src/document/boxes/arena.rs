//! Слоты удерживаемых коробок: записи меняются на месте, ключи переживают правки.

use super::BoxRecord;
use crate::dom::BoxId;

struct Slot {
    generation: u32,
    value: Option<BoxRecord>,
}

#[derive(Default)]
pub(super) struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl Arena {
    #[cfg(feature = "heap-report")]
    pub(super) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        use kamin_heap::HeapSize;
        ops.buffer(self.slots.as_ptr(), self.slots.capacity())
            + self.free.heap_size(ops)
            + self
                .iter()
                .map(|(_, value)| value.heap_size(ops))
                .sum::<usize>()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (BoxId, &BoxRecord)> {
        self.slots.iter().enumerate().filter_map(|(slot, entry)| {
            entry.value.as_ref().map(|value| {
                (
                    BoxId {
                        slot: slot as u32,
                        generation: entry.generation,
                    },
                    value,
                )
            })
        })
    }

    pub(super) fn insert(&mut self, value: BoxRecord) -> Option<BoxId> {
        if let Some(slot) = self.free.pop() {
            let entry = &mut self.slots[slot as usize];
            entry.value = Some(value);
            return Some(BoxId {
                slot,
                generation: entry.generation,
            });
        }
        let slot = u32::try_from(self.slots.len()).ok()?;
        self.slots.push(Slot {
            generation: 1,
            value: Some(value),
        });
        Some(BoxId {
            slot,
            generation: 1,
        })
    }

    pub(super) fn get(&self, id: BoxId) -> Option<&BoxRecord> {
        let slot = self.slots.get(id.slot as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }

    pub(super) fn prepared(&mut self, id: BoxId, prepared: Option<super::finish::Prepared>) {
        let Some(slot) = self.slots.get_mut(id.slot as usize) else {
            return;
        };
        if slot.generation == id.generation
            && let Some(record) = slot.value.as_mut()
        {
            record.prepared = prepared.map(Box::new);
        }
    }

    pub(super) fn prepared_ready(&mut self, id: BoxId, ready: bool) {
        if let Some(slot) = self.slots.get_mut(id.slot as usize)
            && slot.generation == id.generation
            && let Some(record) = slot.value.as_mut()
            && let Some(prepared) = record.prepared.as_mut()
        {
            prepared.ready = ready;
        }
    }

    /// Новое строчное содержимое коробки; `true`, если оно изменилось.
    pub(super) fn inline(
        &mut self,
        id: BoxId,
        content: Option<std::rc::Rc<super::inline::InlineContent>>,
    ) -> bool {
        let Some(record) = self.get_mut(id) else {
            return false;
        };
        // Неизменный вход сохраняет Rc: потребитель может удержать его
        // через commit, а resize не требует повторного сбора текста.
        let changed = record.inline != content;
        if changed {
            record.inline = content;
            record.prepared = None;
        }
        changed
    }

    /// Отцепить детей коробки перед повторной сборкой её списка.
    pub(super) fn unlink_first(&mut self, id: BoxId) {
        if let Some(record) = self.get_mut(id) {
            record.first = BoxId::NONE;
        }
    }

    /// Запись по живому ключу: только для полей, которыми владеет само дерево
    /// (эпохи, признак вклада, слот раскладки), без смены топологии.
    pub(super) fn get_mut(&mut self, id: BoxId) -> Option<&mut BoxRecord> {
        let slot = self.slots.get_mut(id.slot as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.value.as_mut())
            .flatten()
    }

    pub(super) fn remove(&mut self, id: BoxId) -> Option<BoxRecord> {
        let slot = self.slots.get_mut(id.slot as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let value = slot.value.take()?;
        // Переполненный слот уходит навсегда: старый ключ никогда не воскреснет.
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            self.free.push(id.slot);
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(source: u64) -> BoxRecord {
        BoxRecord {
            parent: BoxId::NONE,
            first: BoxId::NONE,
            next: BoxId::NONE,
            source,
            origin: super::super::source::Source::Node(super::super::source::NodeKey::from_packed(
                source,
            )),
            inline: None,
            prepared: None,
            epochs: Default::default(),
            external: false,
            layout_slot: None,
        }
    }

    #[test]
    fn mutation_keeps_the_slot_and_stale_keys_never_resolve() {
        let mut arena = Arena::default();
        let first = arena.insert(record(1)).expect("slot");
        arena.get_mut(first).expect("live").epochs.layout = 7;
        assert_eq!(arena.get(first).map(|r| r.epochs.layout), Some(7));
        assert_eq!(arena.remove(first).map(|r| r.source), Some(1));
        let next = arena.insert(record(2)).expect("reused slot");
        assert_eq!(first.slot, next.slot);
        assert_ne!(first.generation, next.generation);
        assert!(arena.get(first).is_none());
        assert!(arena.get_mut(first).is_none());
        assert!(arena.remove(first).is_none());
        assert_eq!(
            arena.get(next).map(|r| (r.source, r.epochs.layout)),
            Some((2, 0))
        );
    }

    #[test]
    fn a_slot_is_retired_at_generation_exhaustion() {
        let mut arena = Arena::default();
        let id = arena.insert(record(7)).expect("slot");
        arena.slots[id.slot as usize].generation = u32::MAX;
        let exhausted = BoxId {
            generation: u32::MAX,
            ..id
        };
        assert_eq!(arena.remove(exhausted).map(|r| r.source), Some(7));
        let next = arena.insert(record(9)).expect("another slot");
        assert_ne!(next.slot, exhausted.slot);
        assert!(arena.get(exhausted).is_none());
    }
}
