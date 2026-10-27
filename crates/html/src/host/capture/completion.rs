//! Одноразовое ожидание с отменой при закрытии оконного endpoint.

use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};

/// Окно закрылось либо endpoint был заменён.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureClosed;

#[derive(Default)]
struct Event {
    result: Option<Result<u64, CaptureClosed>>,
    waker: Option<Waker>,
}

pub struct Completion(Rc<RefCell<Event>>);
pub(super) struct Waiter {
    after: u64,
    event: Weak<RefCell<Event>>,
}

impl Completion {
    pub(super) fn new(after: u64, result: Option<Result<u64, CaptureClosed>>) -> (Self, Waiter) {
        let event = Rc::new(RefCell::new(Event {
            result,
            waker: None,
        }));
        let waiter = Waiter {
            after,
            event: Rc::downgrade(&event),
        };
        (Self(event), waiter)
    }
}

impl Future for Completion {
    type Output = Result<u64, CaptureClosed>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut event = self.0.borrow_mut();
        if let Some(result) = event.result {
            return Poll::Ready(result);
        }
        event.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl Waiter {
    pub(super) fn alive(&self) -> bool {
        self.event.strong_count() != 0
    }

    /// Вернуть true только для живого ожидания ещё не наступившего sequence.
    pub(super) fn complete(
        &self,
        result: Result<u64, CaptureClosed>,
        wakes: &mut Vec<Waker>,
    ) -> bool {
        let Some(event) = self.event.upgrade() else {
            return false;
        };
        if result.is_ok_and(|seq| seq <= self.after) {
            return true;
        }
        let mut event = event.borrow_mut();
        event.result = Some(result);
        if let Some(waker) = event.waker.take() {
            wakes.push(waker);
        }
        false
    }
}
