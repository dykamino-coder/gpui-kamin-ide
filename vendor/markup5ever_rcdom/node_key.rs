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

struct Slot {
    node: Weak<Node>,
    generation: u32,
    next: u32,
}

struct Slots {
    values: Vec<Slot>,
    lease: Lease,
    free: u32,
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
        let slot = if slots.free != NONE {
            let index = slots.free;
            slots.free = slots.values[index as usize].next;
            index
        } else {
            let index = u32::try_from(slots.values.len()).map_err(|_| policy.overflow())?;
            // NONE зарезервирован как конец списка свободных слотов.
            if index == NONE {
                return Err(policy.overflow());
            }
            let Slots { values, lease, .. } = &mut *slots;
            admission::reserve(values, lease, 1, policy)?;
            values.push(Slot {
                node: Weak::new(),
                generation: 1,
                next: NONE,
            });
            index
        };
        let key = NodeKey {
            slot,
            generation: slots.values[slot as usize].generation,
        };
        Ok(Identity {
            key,
            registry: self.clone(),
        })
    }

    /// Проверка границ, поколения и upgrade: O(1), без обхода дерева.
    pub fn resolve(&self, key: NodeKey) -> Option<Handle> {
        let slots = self.slots.borrow();
        let slot = slots.values.get(key.slot as usize)?;
        (key.generation != 0 && key.generation == slot.generation)
            .then(|| slot.node.upgrade())
            .flatten()
    }

    pub(crate) fn bind(&self, node: &Handle) {
        self.slots.borrow_mut().values[node.key().slot as usize].node = Rc::downgrade(node);
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
                slots.values.capacity() * mem::size_of::<Slot>(),
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
        let free = slots.free;
        let slot = &mut slots.values[self.key.slot as usize];
        debug_assert_eq!(slot.generation, self.key.generation);
        slot.node = Weak::new();
        // Переполнение не возвращает слот в оборот: старый ключ не воскресает.
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            slot.next = free;
            slots.free = self.key.slot;
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
