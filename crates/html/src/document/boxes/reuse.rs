//! Сопоставление старых коробок источникам без отдельного блока для одиночки.

use super::source::Source;
use crate::dom::BoxId;
use std::collections::{HashMap, VecDeque, hash_map::Entry};

enum Slots {
    One(BoxId),
    Fragments(VecDeque<BoxId>),
}

#[derive(Default)]
pub(super) struct Reuse {
    sources: HashMap<Source, Slots>,
}

impl Reuse {
    pub(super) fn insert(&mut self, source: Source, id: BoxId) {
        match self.sources.entry(source) {
            Entry::Vacant(entry) => {
                entry.insert(Slots::One(id));
            }
            Entry::Occupied(entry) => {
                let slots: &mut Slots = entry.into_mut();
                match slots {
                    Slots::One(first) => {
                        *slots = Slots::Fragments(VecDeque::from([*first, id]));
                    }
                    Slots::Fragments(ids) => ids.push_back(id),
                }
            }
        }
    }

    pub(super) fn take(&mut self, source: &Source) -> Option<BoxId> {
        match self.sources.get_mut(source)? {
            Slots::One(id) => {
                let id = std::mem::replace(id, BoxId::NONE);
                id.is_some().then_some(id)
            }
            Slots::Fragments(ids) => ids.pop_front(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::NodeKey;

    #[test]
    fn interleaved_sources_keep_fragment_order_without_reusing_a_slot_twice() {
        let source = |key: u64| Source::Node(NodeKey::from_packed(key));
        let id = |slot| BoxId {
            slot,
            generation: 1,
        };
        let mut reuse = Reuse::default();
        reuse.insert(source(1), id(5));
        reuse.insert(source(2), id(6));
        reuse.insert(source(1), id(7));
        reuse.insert(source(1), id(8));
        assert_eq!(reuse.take(&source(2)), Some(id(6)));
        assert_eq!(reuse.take(&source(1)), Some(id(5)));
        assert_eq!(reuse.take(&source(2)), None);
        assert_eq!(reuse.take(&source(1)), Some(id(7)));
        assert_eq!(reuse.take(&source(1)), Some(id(8)));
        assert_eq!(reuse.take(&source(1)), None);
        assert_eq!(reuse.take(&source(3)), None);
    }
}
