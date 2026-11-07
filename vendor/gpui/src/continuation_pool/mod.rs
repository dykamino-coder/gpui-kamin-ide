//! Переиспользуемое закреплённое хранилище продолжений одного окна.
use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error},
    cell::RefCell,
    future::Future,
    pin::Pin,
    ptr::{self, NonNull},
    rc::Rc,
    task::{Context, Poll},
};

/// Повторно использует слоты завершённых соседей, сохраняя адреса предков.
#[derive(Clone, Default)]
pub struct ContinuationPool(Rc<RefCell<Storage>>);

/// Заимствованное продолжение со стабильным адресом до конца деструктора.
pub struct PaintFuture<'a, T = ()> {
    future: NonNull<dyn Future<Output = T> + 'a>,
    _slot: Slot,
}

impl ContinuationPool {
    /// Закрепляет future в хранилище без продления времени заимствований.
    pub fn pin<'a, F: Future + 'a>(self, future: F) -> PaintFuture<'a, F::Output> {
        let layout = Layout::new::<F>();
        let class = layout
            .size()
            .max(layout.align())
            .max(size_of::<*mut u8>())
            .checked_next_power_of_two()
            .expect("continuation class");
        let index = class.trailing_zeros() as usize;
        let address = self.0.borrow_mut().acquire(index, class);
        // SAFETY: эксклюзивный слот вмещает F с нужным выравниванием.
        // Slot владеет неподвижным выделением до уничтожения F.
        let future = unsafe {
            ptr::write(address.cast::<F>(), future);
            let erased: *mut (dyn Future<Output = F::Output> + 'a) = address.cast::<F>();
            NonNull::new_unchecked(erased)
        };
        PaintFuture {
            future,
            _slot: Slot {
                pool: self,
                address,
                index,
            },
        }
    }

    /// Сохраняет ёмкость по пиковому числу одновременно живых продолжений.
    /// Свободная корзина сжимается после менее глубокого кадра без выделений.
    pub(crate) fn finish_frame(&self) {
        for bucket in &mut self.0.borrow_mut().buckets {
            if bucket.live != 0 {
                continue;
            }
            let mut keep = 0;
            let mut slots = 0;
            while slots < bucket.peak {
                slots += bucket.chunks[keep].slots;
                keep += 1;
            }
            bucket.peak = 0;
            if keep == bucket.chunks.len() {
                continue;
            }
            bucket.head = ptr::null_mut();
            bucket.chunks.truncate(keep);
            bucket.slots = slots;
            for chunk in &bucket.chunks {
                let class = chunk.layout.align();
                for slot in (0..chunk.slots).rev() {
                    // SAFETY: живых future нет; каждая ссылка помещается
                    // в сохранённый свободный слот с нужным выравниванием.
                    unsafe {
                        let address = chunk.address.as_ptr().add(slot * class);
                        address.cast::<*mut u8>().write(bucket.head);
                        bucket.head = address;
                    }
                }
            }
        }
    }
}

impl<T> Future for PaintFuture<'_, T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        // SAFETY: перемещение владельца не меняет адрес слота. Изменяемая
        // ссылка на него существует только внутри poll или деструктора.
        unsafe { Pin::new_unchecked(self.get_mut().future.as_mut()).poll(cx) }
    }
}

impl<T> Drop for PaintFuture<'_, T> {
    fn drop(&mut self) {
        // SAFETY: future инициализирован один раз, закреплён и имеет одного
        // владельца. Slot возвращает память только после этого drop,
        // в том числе при раскрутке стека из деструктора future.
        unsafe { ptr::drop_in_place(self.future.as_ptr()) };
    }
}

struct Slot {
    pool: ContinuationPool,
    address: *mut u8,
    index: usize,
}
impl Drop for Slot {
    fn drop(&mut self) {
        let mut storage = self.pool.0.borrow_mut();
        let bucket = &mut storage.buckets[self.index];
        // SAFETY: future уничтожен, эксклюзивный слот вмещает указатель
        // с его выравниванием. Ссылки хранятся только в свободных слотах.
        unsafe { self.address.cast::<*mut u8>().write(bucket.head) };
        bucket.head = self.address;
        bucket.live -= 1;
    }
}

#[derive(Default)]
struct Storage {
    buckets: Vec<Bucket>,
}
impl Storage {
    fn acquire(&mut self, index: usize, class: usize) -> *mut u8 {
        self.buckets
            .resize_with(self.buckets.len().max(index + 1), Bucket::default);
        let bucket = &mut self.buckets[index];
        if bucket.head.is_null() {
            // Рост определяется одновременно приостановленными продолжениями.
            // Первый слот не резервирует целый блок для будущих соседей.
            let count = bucket.slots.clamp(1, 64);
            let layout = Layout::from_size_align(
                class.checked_mul(count).expect("continuation chunk"),
                class,
            )
            .expect("continuation layout");
            bucket.chunks.reserve(1);
            // SAFETY: ненулевой layout вмещает все слоты с их выравниванием.
            let address = unsafe { alloc(layout) };
            let Some(address) = NonNull::new(address) else {
                handle_alloc_error(layout)
            };
            bucket.chunks.push(Chunk {
                address,
                layout,
                slots: count,
            });
            bucket.slots += count;
            for slot in (0..count).rev() {
                // SAFETY: выровненные ссылки помещаются внутри нового блока;
                // ни в один из его слотов ещё не записан future.
                unsafe {
                    let address = address.as_ptr().add(slot * class);
                    address.cast::<*mut u8>().write(bucket.head);
                    bucket.head = address;
                }
            }
        }
        let address = bucket.head;
        // SAFETY: head указывает на инициализированную ссылку свободного слота.
        bucket.head = unsafe { address.cast::<*mut u8>().read() };
        bucket.live += 1;
        bucket.peak = bucket.peak.max(bucket.live);
        address
    }
}

#[derive(Default)]
struct Bucket {
    head: *mut u8,
    live: usize,
    peak: usize,
    slots: usize,
    chunks: Vec<Chunk>,
}
struct Chunk {
    address: NonNull<u8>,
    layout: Layout,
    slots: usize,
}
impl Drop for Chunk {
    fn drop(&mut self) {
        // SAFETY: блоки освобождаются при пустой корзине или исчезновении
        // последнего владельца пула; каждый Slot владеет ссылкой на пул.
        unsafe { dealloc(self.address.as_ptr(), self.layout) };
    }
}

#[cfg(test)]
mod tests;
