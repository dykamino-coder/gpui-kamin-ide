//! Владение, закрепление и ограниченное переиспользование памяти продолжений.
use super::*;
use std::{cell::Cell, marker::PhantomPinned, panic::AssertUnwindSafe};

fn poll<T>(future: &mut PaintFuture<'_, T>) -> Poll<T> {
    Pin::new(future).poll(&mut Context::from_waker(std::task::Waker::noop()))
}

#[test]
fn siblings_reuse_one_slot_and_borrows_end_when_dropped() {
    let pool = ContinuationPool::default();
    let mut value = 0;
    for _ in 0..16_000 {
        let mut future = pool.clone().pin(async {
            value += 1;
            value
        });
        assert!(poll(&mut future).is_ready());
    }
    assert_eq!(value, 16_000);
    let storage = pool.0.borrow();
    assert_eq!(storage.buckets.iter().map(|b| b.slots).sum::<usize>(), 1);
    assert_eq!(storage.buckets.iter().map(|b| b.live).sum::<usize>(), 0);
}

#[repr(align(64))]
struct Anchored {
    address: Option<NonNull<Self>>,
    _pinned: PhantomPinned,
}
impl Future for Anchored {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        // SAFETY: меняется только сохранённый адрес; закреплённые поля неподвижны.
        let this = unsafe { self.get_unchecked_mut() };
        let address = NonNull::from(&mut *this);
        assert_eq!(address.as_ptr() as usize % 64, 0);
        if let Some(previous) = this.address {
            assert_eq!(previous, address);
            Poll::Ready(())
        } else {
            this.address = Some(address);
            Poll::Pending
        }
    }
}

#[test]
fn pinned_aligned_futures_survive_handle_and_bucket_moves() {
    let pool = ContinuationPool::default();
    let mut futures = Vec::new();
    for _ in 0..20_000 {
        let mut future = pool.clone().pin(Anchored {
            address: None,
            _pinned: PhantomPinned,
        });
        assert!(poll(&mut future).is_pending());
        futures.push(future);
    }
    pool.finish_frame();
    for mut future in futures {
        assert!(poll(&mut future).is_ready());
    }
    pool.finish_frame();
    pool.finish_frame();
    assert!(pool.0.borrow().buckets.iter().all(|b| b.chunks.is_empty()));
}

struct OnDrop {
    pool: ContinuationPool,
    drops: Rc<Cell<usize>>,
    panic: bool,
}
impl Drop for OnDrop {
    fn drop(&mut self) {
        // Выделение и уничтожение других продолжений идут вне заимствования пула.
        let _other = self.pool.clone().pin(std::future::ready(()));
        self.drops.set(self.drops.get() + 1);
        assert!(!self.panic, "drop regression");
    }
}

#[test]
fn cancelled_and_panicking_destructors_return_their_slots() {
    let pool = ContinuationPool::default();
    let drops = Rc::new(Cell::new(0));
    for panic in [false, true] {
        let captured = OnDrop {
            pool: pool.clone(),
            drops: drops.clone(),
            panic,
        };
        let future = pool.clone().pin(async move {
            let _captured = captured;
            std::future::pending::<()>().await;
        });
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| drop(future)));
        assert_eq!(result.is_err(), panic);
        assert!(pool.0.borrow().buckets.iter().all(|b| b.live == 0));
    }
    assert_eq!(drops.get(), 2);
}

#[test]
fn future_keeps_its_storage_alive_after_the_owner_drops() {
    let pool = ContinuationPool::default();
    let owner = Rc::downgrade(&pool.0);
    let mut future = pool.clone().pin(async { 17 });
    drop(pool);
    assert!(owner.upgrade().is_some());
    assert_eq!(poll(&mut future), Poll::Ready(17));
    drop(future);
    assert!(owner.upgrade().is_none());
}

#[test]
fn capacity_tracks_concurrent_frame_demand_instead_of_visited_width() {
    let pool = ContinuationPool::default();
    let futures: Vec<_> = (0..20_000)
        .map(|_| pool.clone().pin(std::future::ready(())))
        .collect();
    drop(futures);
    pool.finish_frame();
    assert!(
        pool.0
            .borrow()
            .buckets
            .iter()
            .map(|b| b.slots)
            .sum::<usize>()
            >= 20_000
    );
    let future = pool.clone().pin(std::future::ready(()));
    drop(future);
    pool.finish_frame();
    assert_eq!(
        pool.0
            .borrow()
            .buckets
            .iter()
            .map(|b| b.slots)
            .sum::<usize>(),
        1
    );
    for _ in 0..16_000 {
        drop(pool.clone().pin(std::future::ready(())));
    }
    pool.finish_frame();
    assert_eq!(
        pool.0
            .borrow()
            .buckets
            .iter()
            .map(|b| b.slots)
            .sum::<usize>(),
        1
    );
}
