//! Кэш одного прохода materialize: одинаковый каскад, родитель и query-container input.
//! Ключ держит владельцев групп, поэтому повторное использование адресов исключено.
use super::ComputedSlot;
use crate::style::computed::{Computed, Storage};
use crate::style::publication::{ComputedBuilder, Published};
use crate::values::ValueCx;
use std::rc::Rc;

const CAPACITY: usize = 32;

#[derive(Default)]
pub(super) struct Sharing {
    entries: [Option<Entry>; CAPACITY],
    next: usize,
}

struct Entry {
    own: Storage,
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
        let recent = (1..=CAPACITY).map(|back| (self.next + CAPACITY - back) % CAPACITY);
        if let Some(hit) = recent
            .filter_map(|i| self.entries[i].as_ref())
            .find(|entry| {
                entry.size == size
                    && entry.own.matches(own)
                    && super::inherited::parent_eq(&entry.parent, parent, own)
            })
        {
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
        self.entries[self.next] = Some(Entry {
            own: Storage::of(own),
            parent: parent.clone(),
            size,
            value: value.clone(),
        });
        self.next = (self.next + 1) % CAPACITY;
        value
    }
}

#[cfg(test)]
mod tests;
