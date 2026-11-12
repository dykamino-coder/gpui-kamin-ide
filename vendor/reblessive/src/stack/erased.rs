//! A stack future whose type names only its output (KaminIDE).
//!
//! [`Stk::run`](super::Stk::run) returns a future that names the closure, so a
//! function pointer that starts a nested future on the stack has to box it to
//! return it. [`Stk::run_erased`](super::Stk::run_erased) keeps the same
//! future inline in a fixed buffer and erases its type behind two function
//! pointers, so such a function returns it without an allocation; the nested
//! future itself still goes on the stack allocator when first polled.

use super::future::InnerStkFuture;
use super::stk::Stk;
use std::{
    future::Future,
    marker::{PhantomData, PhantomPinned},
    mem::{align_of, size_of, MaybeUninit},
    pin::Pin,
    task::{Context, Poll},
};

/// Bytes an erased future keeps inline: the closure that starts the nested
/// future, or the slot of its result.
pub const ERASED_INLINE: usize = 64;

#[repr(C, align(16))]
struct Inline([MaybeUninit<u8>; ERASED_INLINE]);

/// Future returned by [`Stk::run_erased`](super::Stk::run_erased).
///
/// Should be immediately polled when created and driven until finished, like
/// [`StkFuture`](super::StkFuture).
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ErasedStkFuture<'a, R> {
    inline: Inline,
    poll: unsafe fn(*mut u8, &mut Context<'_>) -> Poll<R>,
    drop: unsafe fn(*mut u8),
    _marker: PhantomData<(&'a mut Stk, *mut ())>,
    _pinned: PhantomPinned,
}

/// Compile-time check that `T` fits the inline buffer.
struct Fits<T>(PhantomData<T>);

impl<T> Fits<T> {
    const OK: () = assert!(
        size_of::<T>() <= ERASED_INLINE && align_of::<T>() <= 16,
        "the closure of `Stk::run_erased` does not fit the inline buffer"
    );
}

impl<'a, R> ErasedStkFuture<'a, R> {
    pub(crate) fn new<F, Fut>(f: F) -> Self
    where
        F: FnOnce(&'a mut Stk) -> Fut,
        Fut: Future<Output = R> + 'a,
    {
        #[allow(clippy::let_unit_value)]
        let () = Fits::<InnerStkFuture<'a, F, R, Stk>>::OK;
        let mut inline = Inline([MaybeUninit::uninit(); ERASED_INLINE]);
        // Safety: the buffer is large and aligned enough (`Fits`) and holds
        // nothing yet. The future is not polled before `Self` is pinned, so
        // moving `Self` until then moves a future that holds only `f`.
        unsafe {
            inline
                .0
                .as_mut_ptr()
                .cast::<InnerStkFuture<'a, F, R, Stk>>()
                .write(InnerStkFuture::new(f));
        }
        ErasedStkFuture {
            inline,
            poll: poll_inline::<F, Fut, R>,
            drop: drop_inline::<F, R>,
            _marker: PhantomData,
            _pinned: PhantomPinned,
        }
    }
}

/// # Safety
/// `ptr` points at a live, pinned `InnerStkFuture<'a, F, R, Stk>`.
unsafe fn poll_inline<'a, F, Fut, R>(ptr: *mut u8, cx: &mut Context<'_>) -> Poll<R>
where
    F: FnOnce(&'a mut Stk) -> Fut,
    Fut: Future<Output = R> + 'a,
{
    Pin::new_unchecked(&mut *ptr.cast::<InnerStkFuture<'a, F, R, Stk>>()).poll(cx)
}

/// # Safety
/// `ptr` points at a live `InnerStkFuture<'_, F, R, Stk>` that is not used after.
unsafe fn drop_inline<F, R>(ptr: *mut u8) {
    std::ptr::drop_in_place(ptr.cast::<InnerStkFuture<'_, F, R, Stk>>());
}

impl<R> Future for ErasedStkFuture<'_, R> {
    type Output = R;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<R> {
        // Safety: `self` is pinned, so is the future in its buffer, which was
        // written by `new` with the type `poll` was made for.
        unsafe {
            let this = self.get_unchecked_mut();
            (this.poll)(this.inline.0.as_mut_ptr().cast(), cx)
        }
    }
}

impl<R> Drop for ErasedStkFuture<'_, R> {
    fn drop(&mut self) {
        // Safety: the buffer holds the future `new` wrote, dropped only here.
        unsafe { (self.drop)(self.inline.0.as_mut_ptr().cast()) }
    }
}

#[cfg(test)]
mod tests {
    use super::ErasedStkFuture;
    use crate::{Stack, Stk};
    use std::{cell::Cell, rc::Rc};

    type Start = for<'a> fn(&'a mut Stk, usize) -> ErasedStkFuture<'a, usize>;

    fn start(stk: &mut Stk, depth: usize) -> ErasedStkFuture<'_, usize> {
        stk.run_erased(move |stk| descend(stk, depth))
    }

    async fn descend(stk: &mut Stk, depth: usize) -> usize {
        let start: Start = start;
        match depth {
            0 => 0,
            _ => start(stk, depth - 1).await + 1,
        }
    }

    #[test]
    fn a_function_pointer_returns_a_nested_future() {
        let depth = if cfg!(miri) { 16 } else { 100_000 };
        let mut stack = Stack::new();
        assert_eq!(stack.enter(|stk| descend(stk, depth)).finish(), depth);
    }

    struct Counted(Rc<Cell<u32>>);

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn a_future_dropped_unpolled_drops_its_closure() {
        let drops = Rc::new(Cell::new(0));
        let counted = Counted(drops.clone());
        let mut stack = Stack::new();
        let out = stack
            .enter(|stk| async move {
                let unpolled = stk.run_erased(move |_| async move {
                    drop(counted);
                    1
                });
                drop(unpolled);
                7
            })
            .finish();
        assert_eq!(out, 7);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn a_future_moved_before_its_first_poll_runs() {
        let mut stack = Stack::new();
        let out = stack
            .enter(|stk| async move {
                let v = vec![1u8, 2, 3];
                let fut = stk.run_erased(move |stk| async move {
                    stk.yield_now().await;
                    v.len()
                });
                Box::into_pin(Box::new(fut)).await
            })
            .finish();
        assert_eq!(out, 3);
    }

    #[test]
    fn a_panic_in_the_nested_future_reaches_the_caller() {
        let caught = std::panic::catch_unwind(|| {
            let mut stack = Stack::new();
            stack
                .enter(|stk| async move {
                    stk.run_erased(|_| async {
                        if true {
                            panic!("nested");
                        }
                        0u8
                    })
                    .await
                })
                .finish()
        });
        assert!(caught.is_err());
    }

    #[test]
    fn erased_and_plain_futures_nest() {
        async fn plain(stk: &mut Stk, n: u32) -> u32 {
            if n == 0 {
                return 0;
            }
            stk.run_erased(move |stk| async move { stk.run(move |stk| plain(stk, n - 1)).await + 1 })
                .await
        }
        let mut stack = Stack::new();
        assert_eq!(stack.enter(|stk| plain(stk, 64)).finish(), 64);
    }
}