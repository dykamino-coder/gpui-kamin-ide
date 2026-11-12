//! Размеры кучи типов std: строки, контейнеры, `Box`, `Rc`/`Arc`, хэш-таблицы.

use super::{HeapSize, Ops};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, HashSet},
    mem::{align_of_val, size_of, size_of_val},
    rc::Rc,
    sync::Arc,
};

no_heap!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    std::time::Duration,
    std::time::Instant,
);

impl<T: ?Sized> HeapSize for std::marker::PhantomData<T> {
    fn heap_size(&self, _: &mut Ops) -> usize {
        0
    }
}

impl HeapSize for String {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.block(self.as_ptr(), self.capacity())
    }
}

impl HeapSize for str {
    fn heap_size(&self, _: &mut Ops) -> usize {
        0
    }
}

impl<T: HeapSize> HeapSize for [T] {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.iter().map(|item| item.heap_size(ops)).sum()
    }
}

impl<T: HeapSize, const N: usize> HeapSize for [T; N] {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.as_slice().heap_size(ops)
    }
}

impl<T: HeapSize> HeapSize for Vec<T> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.buffer(self.as_ptr(), self.capacity()) + self.as_slice().heap_size(ops)
    }
}

impl<T: HeapSize + ?Sized> HeapSize for Box<T> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        let value: &T = self;
        ops.block((value as *const T).cast(), size_of_val(value)) + value.heap_size(ops)
    }
}

impl<T: HeapSize> HeapSize for Option<T> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.as_ref().map_or(0, |value| value.heap_size(ops))
    }
}

impl<T: HeapSize, E: HeapSize> HeapSize for Result<T, E> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        match self {
            Ok(value) => value.heap_size(ops),
            Err(error) => error.heap_size(ops),
        }
    }
}

impl<T: Copy> HeapSize for Cell<T> {
    fn heap_size(&self, _: &mut Ops) -> usize {
        0
    }
}

impl<T: HeapSize> HeapSize for std::cell::OnceCell<T> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.get().map_or(0, |value| value.heap_size(ops))
    }
}

impl<T: HeapSize + ?Sized> HeapSize for RefCell<T> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        // Заимствованное на запись сейчас меряет его владелец.
        self.try_borrow().map_or(0, |value| value.heap_size(ops))
    }
}

/// Заголовок счётчиков `Rc`/`Arc` перед значением: два слова, выровненные
/// по значению.
pub(crate) fn counted_header(align: usize) -> usize {
    (2 * size_of::<usize>()).next_multiple_of(align.max(align_of::<usize>()))
}

fn counted<T: HeapSize + ?Sized>(value: &T, ops: &mut Ops) -> usize {
    let pointer = value as *const T as *const u8;
    if !ops.once(pointer) {
        return 0;
    }
    let align = align_of_val(value).max(align_of::<usize>());
    let header = counted_header(align);
    let bytes = (header + size_of_val(value)).next_multiple_of(align);
    // Только адрес начала блока Rc/Arc для приёмника; не разыменуется.
    let start = pointer.wrapping_sub(header);
    ops.block(start, bytes) + value.heap_size(ops)
}

impl<T: HeapSize + ?Sized> HeapSize for Rc<T> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        counted(&**self, ops)
    }
}

impl<T: HeapSize + ?Sized> HeapSize for Arc<T> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        counted(&**self, ops)
    }
}

impl<T: ?Sized> HeapSize for std::rc::Weak<T> {
    fn heap_size(&self, _: &mut Ops) -> usize {
        0
    }
}

impl<K: HeapSize, V: HeapSize, S> HeapSize for HashMap<K, V, S> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.table::<(K, V)>(self.capacity())
            + self
                .iter()
                .map(|(key, value)| key.heap_size(ops) + value.heap_size(ops))
                .sum::<usize>()
    }
}

impl<T: HeapSize, S> HeapSize for HashSet<T, S> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        ops.table::<T>(self.capacity()) + self.iter().map(|item| item.heap_size(ops)).sum::<usize>()
    }
}

impl<K: HeapSize, V: HeapSize> HeapSize for BTreeMap<K, V> {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        // Узлы B-дерева: до 11 записей на узел; оценка — заполненность 2/3.
        let nodes = self.len().div_ceil(7);
        let node = 11 * (size_of::<K>() + size_of::<V>()) + 4 * size_of::<usize>();
        ops.block(std::ptr::null(), nodes * node)
            + self
                .iter()
                .map(|(key, value)| key.heap_size(ops) + value.heap_size(ops))
                .sum::<usize>()
    }
}

macro_rules! tuples {
    ($(($($name:ident),+)),*) => {
        $(impl<$($name: HeapSize),+> HeapSize for ($($name,)+) {
            #[allow(non_snake_case)]
            fn heap_size(&self, ops: &mut Ops) -> usize {
                let ($($name,)+) = self;
                0 $(+ $name.heap_size(ops))+
            }
        })*
    };
}
tuples!((A), (A, B), (A, B, C), (A, B, C, D));
