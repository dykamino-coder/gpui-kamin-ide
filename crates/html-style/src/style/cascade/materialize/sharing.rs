//! Кэш одного прохода materialize: одинаковый каскад, родитель и query-container input.
//! Ключ держит владельцев групп, поэтому повторное использование адресов исключено.
use super::ComputedSlot;
use crate::style::computed::Computed;
use crate::style::publication::{ComputedBuilder, Published};
use crate::values::ValueCx;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct Sharing {
    entries: VecDeque<Entry>,
}

struct Entry {
    own: Computed,
    parent: Rc<Computed>,
    size: (Option<u32>, Option<u32>),
    value: Published,
}

pub(super) fn size_key(size: (Option<f32>, Option<f32>)) -> (Option<u32>, Option<u32>) {
    (size.0.map(f32::to_bits), size.1.map(f32::to_bits))
}

impl Sharing {
    pub(super) fn compute(
        &mut self,
        slot: &ComputedSlot,
        parent: &Rc<Computed>,
        own: &Computed,
        vcx: ValueCx<'_>,
    ) -> Published {
        let size = size_key(slot.1.get());
        if let Some(hit) = self.entries.iter().rev().find(|entry| {
            entry.size == size
                && (Rc::ptr_eq(&entry.parent, parent) || entry.parent.shares_storage(parent))
                && entry.own.shares_storage(own)
        }) {
            crate::work::add!(MaterializedSharingHits, 1);
            if hit.value.decor_box == own.decor_box {
                return hit.value.clone();
            }
            // DOM id нужен стадии коробки; computed CSS от него не зависит.
            let mut value = ComputedBuilder::from_cascaded(&hit.value);
            value.decor_box = own.decor_box;
            return value.finish();
        }
        let value = slot.compute(parent, own, vcx);
        if self.entries.len() == 32 {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry {
            own: own.clone(),
            parent: parent.clone(),
            size,
            value: value.clone(),
        });
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::css::Media;
    use crate::values::DocumentValues;

    #[test]
    fn sharing_requires_same_parent_groups_and_container_input() {
        let doc = DocumentValues::detached(Media::default());
        let vcx = doc.values();
        let mut cache = Sharing::default();
        let parent = Rc::new(Computed::default());
        let own = Computed::default();
        let first_slot = ComputedSlot::default();
        let first = cache.compute(&first_slot, &parent, &own, vcx);
        let second = cache.compute(&ComputedSlot::default(), &parent, &own.clone(), vcx);
        assert!(first.handle.ptr_eq(&second.handle));
        let mut changed_parent = (*parent).clone();
        changed_parent.reset_effects.opacity = Some(0.5);
        let distinct_parent = Rc::new(changed_parent);
        let third = cache.compute(&first_slot, &distinct_parent, &own, vcx);
        assert!(!first.handle.ptr_eq(&third.handle));
        first_slot.set_container_size((Some(50.0), None));
        let fourth = cache.compute(&first_slot, &parent, &own, vcx);
        assert!(!first.handle.ptr_eq(&fourth.handle));
        let mut changed = own.clone();
        changed.reset_box_layout.width = Some(crate::style::values::value::Len::Px(10.0));
        let fifth = cache.compute(&ComputedSlot::default(), &parent, &changed, vcx);
        assert!(!first.handle.ptr_eq(&fifth.handle));
    }

    #[test]
    fn decoration_identity_keeps_css_groups_shared() {
        let doc = DocumentValues::detached(Media::default());
        let parent = Rc::new(Computed::default());
        let mut own = Computed::default();
        own.decor_box = 10;
        let mut cache = Sharing::default();
        let first = cache.compute(&ComputedSlot::default(), &parent, &own, doc.values());
        own.decor_box = 20;
        let mut cousin_parent = (*parent).clone();
        cousin_parent.decor_box = 99;
        let second = cache.compute(
            &ComputedSlot::default(),
            &Rc::new(cousin_parent),
            &own,
            doc.values(),
        );
        assert_eq!(first.decor_box, 10);
        assert_eq!(second.decor_box, 20);
        assert!(first.shares_storage(&second));
    }

    #[test]
    fn size_key_preserves_float_bits() {
        assert_ne!(size_key((Some(0.0), None)), size_key((Some(-0.0), None)));
    }
}
