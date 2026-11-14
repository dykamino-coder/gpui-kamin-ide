//! Слоты удерживаемых коробок: записи меняются на месте, ключи переживают правки.

use crate::dom::BoxId;

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

pub(super) struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T> Arena<T> {
    pub(super) fn iter(&self) -> impl Iterator<Item = (BoxId, &T)> {
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

    pub(super) fn insert(&mut self, value: T) -> Option<BoxId> {
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

    pub(super) fn get(&self, id: BoxId) -> Option<&T> {
        let slot = self.slots.get(id.slot as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }

    pub(super) fn get_mut(&mut self, id: BoxId) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.slot as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.value.as_mut())
            .flatten()
    }

    pub(super) fn remove(&mut self, id: BoxId) -> Option<T> {
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

    #[test]
    fn mutation_keeps_the_slot_and_stale_keys_never_resolve() {
        let mut arena = Arena::default();
        let first = arena.insert(String::from("before")).expect("slot");
        arena.get_mut(first).expect("live").push_str(" after");
        assert_eq!(arena.get(first).map(String::as_str), Some("before after"));
        assert_eq!(arena.remove(first).as_deref(), Some("before after"));
        let next = arena.insert(String::from("next")).expect("reused slot");
        assert_eq!(first.slot, next.slot);
        assert_ne!(first.generation, next.generation);
        assert!(arena.get(first).is_none());
        assert!(arena.get_mut(first).is_none());
        assert!(arena.remove(first).is_none());
        assert_eq!(arena.get(next).map(String::as_str), Some("next"));
    }

    #[test]
    fn a_slot_is_retired_at_generation_exhaustion() {
        let mut arena = Arena::default();
        let id = arena.insert(7).expect("slot");
        arena.slots[id.slot as usize].generation = u32::MAX;
        let exhausted = BoxId {
            generation: u32::MAX,
            ..id
        };
        assert_eq!(arena.remove(exhausted), Some(7));
        let next = arena.insert(9).expect("another slot");
        assert_ne!(next.slot, exhausted.slot);
        assert!(arena.get(exhausted).is_none());
    }
}
