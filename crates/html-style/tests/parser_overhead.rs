//! Проверка фиксированной reservation по реальным аллокациям BufferQueue.

#[path = "../src/dom/input/overhead.rs"]
mod overhead;

use html5ever::{buffer_queue::BufferQueue, tendril::StrTendril};
use overhead::QUEUE_OVERHEAD;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Measured;

// Измеритель только тестового потока; состояния документа здесь нет.
std::thread_local! {
    static TRACKING: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for Measured {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let _ = TRACKING.try_with(|tracking| {
                if let Some((live, peak)) = tracking.get() {
                    let live = live + layout.size();
                    tracking.set(Some((live, peak.max(live))));
                }
            });
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let _ = TRACKING.try_with(|tracking| {
            if let Some((live, peak)) = tracking.get() {
                tracking.set(Some((live - layout.size(), peak)));
            }
        });
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: Measured = Measured;

#[test]
fn fixed_queue_reservation_covers_actual_markup5ever_constructor_storage() {
    TRACKING.set(Some((0, 0)));
    let queues = std::array::from_fn::<_, 3, _>(|_| BufferQueue::default());
    for queue in &queues {
        for _ in 0..16 {
            queue.push_back(StrTendril::from_slice("a"));
        }
        for _ in 0..16 {
            let _ = queue.next();
        }
    }
    let retained = TRACKING.get().unwrap();
    drop(queues);
    let released = TRACKING.replace(None).unwrap();
    assert_eq!(retained, (QUEUE_OVERHEAD, QUEUE_OVERHEAD));
    assert_eq!(released, (0, QUEUE_OVERHEAD));
}
