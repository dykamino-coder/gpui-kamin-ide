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

/// Публикация меняет только топологию: наружу не выдаётся изменяемая запись.
pub(super) enum Link {
    Reset {
        parent: BoxId,
        source: u64,
        epoch: u64,
    },
    First(BoxId),
    Next(BoxId),
    Epoch(u64),
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
            record.prepared = prepared;
        }
    }

    pub(super) fn link(&mut self, id: BoxId, change: Link) -> bool {
        let Some(slot) = self.slots.get_mut(id.slot as usize) else {
            return false;
        };
        if slot.generation != id.generation {
            return false;
        }
        let Some(record) = slot.value.as_mut() else {
            return false;
        };
        match change {
            Link::Reset {
                parent,
                source,
                epoch,
            } => {
                record.parent = parent;
                record.source = source;
                record.first = BoxId::NONE;
                record.next = BoxId::NONE;
                record.epoch = epoch;
            }
            Link::First(first) => record.first = first,
            Link::Next(next) => record.next = next,
            Link::Epoch(epoch) => record.epoch = epoch,
        }
        true
    }

    pub(super) fn inline(
        &mut self,
        id: BoxId,
        content: Option<std::rc::Rc<super::inline::InlineContent>>,
    ) {
        let Some(slot) = self.slots.get_mut(id.slot as usize) else {
            return;
        };
        if slot.generation != id.generation {
            return;
        }
        if let Some(record) = slot.value.as_mut() {
            // Неизменный вход сохраняет Rc: потребитель может удержать его
            // через commit, а resize не требует повторного сбора текста.
            if record.inline != content {
                record.inline = content;
                record.prepared = None;
            }
        }
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
            origin: super::super::source::Source::Node(super::super::source::NodeKey {
                slot: source as u32,
                generation: 1,
            }),
            inline: None,
            prepared: None,
            epoch: 0,
        }
    }

    #[test]
    fn mutation_keeps_the_slot_and_stale_keys_never_resolve() {
        let mut arena = Arena::default();
        let first = arena.insert(record(1)).expect("slot");
        assert!(arena.link(first, Link::Epoch(7)));
        assert_eq!(arena.get(first).map(|r| r.epoch), Some(7));
        assert_eq!(arena.remove(first).map(|r| r.source), Some(1));
        let next = arena.insert(record(2)).expect("reused slot");
        assert_eq!(first.slot, next.slot);
        assert_ne!(first.generation, next.generation);
        assert!(arena.get(first).is_none());
        assert!(!arena.link(first, Link::Epoch(9)));
        assert!(arena.remove(first).is_none());
        assert_eq!(arena.get(next).map(|r| (r.source, r.epoch)), Some((2, 0)));
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
