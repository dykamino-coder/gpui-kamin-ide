//! Live atlas textures keep unique ids without accumulating empty slots.

use std::collections::BTreeMap;

pub(super) struct TextureSlots<T> {
    live: BTreeMap<u32, T>,
    next_id: Option<u32>,
}

impl<T> Default for TextureSlots<T> {
    fn default() -> Self {
        Self {
            live: BTreeMap::new(),
            next_id: Some(0),
        }
    }
}

impl<T> TextureSlots<T> {
    pub(super) fn next_id(&mut self) -> Option<u32> {
        let id = self.next_id?;
        self.next_id = id.checked_add(1);
        Some(id)
    }

    pub(super) fn insert(&mut self, id: u32, texture: T) -> &mut T {
        self.live.entry(id).or_insert(texture)
    }

    pub(super) fn get(&self, id: u32) -> Option<&T> {
        self.live.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        self.live.get_mut(&id)
    }

    pub(super) fn remove(&mut self, id: u32) {
        self.live.remove(&id);
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &T> {
        self.live.values()
    }

    pub(super) fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> {
        self.live.values_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_ids_do_not_alias_new_textures_or_accumulate_tombstones() {
        let mut slots = TextureSlots::default();
        let first = slots.next_id().unwrap();
        slots.insert(first, "old");
        slots.remove(first);
        for _ in 0..10_000 {
            let id = slots.next_id().unwrap();
            slots.insert(id, "new");
            assert_eq!(slots.live.len(), 1);
            assert!(slots.get(first).is_none());
            slots.remove(id);
            assert!(slots.live.is_empty());
        }
    }

    #[test]
    fn allocation_keeps_reverse_creation_order_after_removal() {
        let mut slots = TextureSlots::default();
        for value in 0..3 {
            let id = slots.next_id().unwrap();
            slots.insert(id, value);
        }
        slots.remove(1);
        assert_eq!(
            slots
                .iter_mut()
                .rev()
                .map(|value| *value)
                .collect::<Vec<_>>(),
            [2, 0]
        );
    }

    #[test]
    fn exhausted_ids_fail_without_wrapping_into_old_scenes() {
        let mut slots = TextureSlots::<()>::default();
        slots.next_id = Some(u32::MAX);
        assert_eq!(slots.next_id(), Some(u32::MAX));
        assert_eq!(slots.next_id(), None);
        assert_eq!(slots.next_id(), None);
    }
}
