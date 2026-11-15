//! Устойчивая невладеющая идентичность исходного DOM до фактического Drop.

use crate::{
    admission::{self, Admission, Lease},
    Handle, Node,
};
use std::{
    cell::RefCell,
    mem,
    rc::{Rc, Weak},
};

/// POD-контракт хоста; ключ имеет смысл только в реестре своего документа.
/// Копирование ключа не удерживает узел. Нулевое поколение недействительно.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct NodeKey {
    pub slot: u32,
    pub generation: u32,
}

impl NodeKey {
    /// Совместимое с деревом коробок представление; 0 оставлен синтетике.
    pub const fn packed(self) -> u64 {
        (self.generation as u64) << 32 | self.slot as u64
    }

    pub const fn from_packed(value: u64) -> Self {
        Self {
            slot: value as u32,
            generation: (value >> 32) as u32,
        }
    }
}

const NONE: u32 = u32::MAX;
/// The largest generation a slot word holds (31 bits beside the tag bit).
const LAST_GENERATION: u32 = u32::MAX >> 1;
/// A slot handed to an identity whose node is not bound yet, or retired.
const RESERVED: u64 = 1;

/// One word per slot. A live node's `Weak` as a raw pointer (aligned, so
/// even), or an odd tag: a free slot is `next << 32 | generation << 1 | 1`
/// (the next free slot and the generation it hands out), `RESERVED` an
/// allocated but unbound or retired one. The generation of a live slot is in
/// its node's key, so the registry spends one word per slot and nothing per
/// node besides it (Blink keeps node ids in a side map from node to id,
/// `DOMNodeIds` over `WeakIdentifierMap`; here the id is in the node and the
/// map from id to node is this dense vector).
struct Slots {
    values: Vec<u64>,
    lease: Lease,
    free: u32,
}

fn free_word(next: u32, generation: u32) -> u64 {
    (next as u64) << 32 | (generation as u64) << 1 | 1
}

impl Slots {
    fn node(&self, slot: u32) -> Option<Handle> {
        let word = *self.values.get(slot as usize)?;
        if word & 1 == 1 {
            return None;
        }
        // SAFETY: an even word is a pointer from `Weak::into_raw` that the
        // slot owns until it is replaced; it is not consumed here.
        let weak = mem::ManuallyDrop::new(unsafe { Weak::from_raw(word as usize as *const Node) });
        weak.upgrade()
    }

    fn release(&mut self, slot: u32) {
        let word = mem::replace(&mut self.values[slot as usize], RESERVED);
        if word & 1 == 0 {
            // SAFETY: the slot owned this `Weak::into_raw` pointer.
            drop(unsafe { Weak::from_raw(word as usize as *const Node) });
        }
    }
}

impl Drop for Slots {
    fn drop(&mut self) {
        for slot in 0..self.values.len() as u32 {
            self.release(slot);
        }
    }
}

/// Слабый реестр разделяется живыми узлами, включая отсоединённые под JS.
/// Сильные ссылки направлены только от узлов к реестру: цикла нет.
pub struct NodeRegistry {
    slots: RefCell<Slots>,
    _lease: Lease,
}

impl NodeRegistry {
    pub(crate) fn new<A: Admission>(policy: &A) -> Result<Rc<Self>, A::Error> {
        let lease = policy.storage(mem::size_of::<Self>() + 2 * mem::size_of::<usize>())?;
        Ok(Rc::new(Self {
            slots: RefCell::new(Slots {
                values: Vec::new(),
                lease: None,
                free: NONE,
            }),
            _lease: lease,
        }))
    }

    pub(crate) fn allocate<A: Admission>(
        self: &Rc<Self>,
        policy: &A,
    ) -> Result<Identity, A::Error> {
        let mut slots = self.slots.borrow_mut();
        let (slot, generation) = if slots.free != NONE {
            let index = slots.free;
            let word = slots.values[index as usize];
            slots.free = (word >> 32) as u32;
            slots.values[index as usize] = RESERVED;
            (index, (word as u32) >> 1)
        } else {
            let index = u32::try_from(slots.values.len()).map_err(|_| policy.overflow())?;
            // NONE зарезервирован как конец списка свободных слотов.
            if index == NONE {
                return Err(policy.overflow());
            }
            let Slots { values, lease, .. } = &mut *slots;
            admission::reserve(values, lease, 1, policy)?;
            values.push(RESERVED);
            (index, 1)
        };
        Ok(Identity {
            key: NodeKey { slot, generation },
            registry: self.clone(),
        })
    }
    /// Проверка границ, поколения и upgrade: O(1), без обхода дерева.
    pub fn resolve(&self, key: NodeKey) -> Option<Handle> {
        if key.generation == 0 {
            return None;
        }
        let node = self.slots.borrow().node(key.slot)?;
        (node.identity.key == key).then_some(node)
    }

    pub(crate) fn bind(&self, node: &Handle) {
        let mut slots = self.slots.borrow_mut();
        let slot = node.key().slot;
        slots.release(slot);
        slots.values[slot as usize] = Weak::into_raw(Rc::downgrade(node)) as usize as u64;
    }
    pub(crate) fn heap_blocks(&self, sink: &mut dyn FnMut(*const u8, usize)) {
        let header = 2 * mem::size_of::<usize>();
        sink(
            (self as *const Self).cast::<u8>().wrapping_sub(header),
            mem::size_of::<Self>() + header,
        );
        let slots = self.slots.borrow();
        if slots.values.capacity() > 0 {
            sink(
                slots.values.as_ptr().cast(),
                slots.values.capacity() * mem::size_of::<u64>(),
            );
        }
        lease_block(sink, &self._lease);
        lease_block(sink, &slots.lease);
    }
}

pub(crate) fn lease_block(sink: &mut dyn FnMut(*const u8, usize), lease: &Lease) {
    if let Some(value) = lease {
        let value: &(dyn std::any::Any + Send + Sync) = &**value;
        let align = mem::align_of_val(value).max(mem::align_of::<usize>());
        let header = (2 * mem::size_of::<usize>()).next_multiple_of(align);
        let bytes = (header + mem::size_of_val(value)).next_multiple_of(align);
        sink(
            (value as *const dyn std::any::Any)
                .cast::<u8>()
                .wrapping_sub(header),
            bytes,
        );
    }
}

pub(crate) struct Identity {
    pub(crate) key: NodeKey,
    pub(crate) registry: Rc<NodeRegistry>,
}

impl Drop for Identity {
    fn drop(&mut self) {
        let mut slots = self.registry.slots.borrow_mut();
        let slot = self.key.slot;
        slots.release(slot);
        // Переполнение не возвращает слот в оборот: старый ключ не воскресает.
        if self.key.generation < LAST_GENERATION {
            let free = slots.free;
            slots.values[slot as usize] = free_word(free, self.key.generation + 1);
            slots.free = slot;
        }
    }
}
impl Node {
    pub fn key(&self) -> NodeKey {
        self.identity.key
    }

    /// Владелец реестра, например для повторного разбора того же документа.
    pub fn registry(&self) -> Rc<NodeRegistry> {
        self.identity.registry.clone()
    }

    pub fn resolve_key(&self, key: NodeKey) -> Option<Handle> {
        self.identity.registry.resolve(key)
    }

    /// Создать узел в том же пространстве ключей; для bounded-кода — TreeSink.
    pub fn new_in(&self, data: crate::NodeData) -> Handle {
        admission::infallible(Node::try_new(
            data,
            None,
            &admission::Unbounded,
            &self.identity.registry,
        ))
    }
}

#[cfg(test)]
mod tests;
