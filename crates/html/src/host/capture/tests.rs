//! Проверки изоляции окон, жизненного цикла подписок и политики кадров.

use super::*;
use futures::{FutureExt, task::noop_waker};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

fn store(sink: &mut dyn FrameCaptureSink, byte: u8) {
    sink.completed(
        CapturedFrame {
            width: 1,
            height: 1,
            bgra: vec![byte; 4],
        },
        Duration::from_micros(7),
    );
}

#[test]
fn first_latest_rearm_and_completion_preserve_the_document_fence() {
    let (session, mut sink) = CaptureSession::open();
    let mut next = session.after(0);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Pending);
    session.arm_first();
    store(&mut *sink, 1);
    assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Ready(Ok(1)));
    store(&mut *sink, 2);
    assert_eq!(session.take_first().unwrap().pixels.bgra, [1; 4]);
    assert!(session.take_first().is_none());
    assert_eq!(session.latest().unwrap().pixels.bgra, [2; 4]);
    assert_eq!(session.after(1).now_or_never(), Some(Ok(2)));
    session.arm_first();
    assert!(session.take_first().is_none());
    store(&mut *sink, 3);
    assert_eq!(session.take_first().unwrap().seq, 3);
    assert_eq!(session.readback_time(), Duration::from_micros(21));
}

#[test]
fn two_windows_never_share_frames_sequences_timing_or_waiters() {
    let (a, mut sink_a) = CaptureSession::open();
    let (b, mut sink_b) = CaptureSession::open();
    a.arm_first();
    b.arm_first();
    let mut event_b = Box::pin(b.after(0));
    store(&mut *sink_a, 1);
    assert!(event_b.as_mut().now_or_never().is_none());
    assert_eq!(b.sequence(), 0);
    assert_eq!(b.readback_time(), Duration::ZERO);
    assert!(b.latest().is_none());
    assert!(b.take_first().is_none());
    store(&mut *sink_b, 2);
    assert_eq!(event_b.now_or_never(), Some(Ok(1)));
    assert_eq!(a.latest().unwrap().pixels.bgra, [1; 4]);
    assert_eq!(b.take_first().unwrap().pixels.bgra, [2; 4]);
}

#[test]
fn endpoint_drop_cancels_waiters_and_a_replacement_starts_independently() {
    let (old, mut sink) = CaptureSession::open();
    store(&mut *sink, 1);
    let pending = old.after(1);
    drop(sink);
    assert_eq!(pending.now_or_never(), Some(Err(CaptureClosed)));
    assert_eq!(old.after(0).now_or_never(), Some(Err(CaptureClosed)));
    assert!(old.latest().is_none());
    let (new, mut replacement) = CaptureSession::open();
    store(&mut *replacement, 2);
    assert_eq!(new.sequence(), 1);
    assert_eq!(old.sequence(), 1);
    assert!(old.latest().is_none());
}

#[test]
fn cancelled_waiters_are_reclaimed_without_another_frame() {
    let (session, _sink) = CaptureSession::open();
    for _ in 0..100 {
        drop(session.after(0));
    }
    assert!(session.0.borrow().waiters.len() <= 1);
}

#[test]
fn completion_and_window_close_wake_registered_tasks() {
    use futures::task::{ArcWake, waker};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct WakeCount(AtomicUsize);
    impl ArcWake for WakeCount {
        fn wake_by_ref(this: &Arc<Self>) {
            this.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let count = Arc::new(WakeCount(AtomicUsize::new(0)));
    let waker = waker(count.clone());
    let mut cx = Context::from_waker(&waker);
    let (session, mut sink) = CaptureSession::open();
    let mut next = session.after(0);
    let mut later = session.after(2);
    assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Pending);
    assert_eq!(Pin::new(&mut later).poll(&mut cx), Poll::Pending);
    store(&mut *sink, 1);
    assert_eq!(count.0.load(Ordering::Relaxed), 1);
    assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Ready(Ok(1)));
    assert_eq!(Pin::new(&mut later).poll(&mut cx), Poll::Pending);
    drop(sink);
    assert_eq!(count.0.load(Ordering::Relaxed), 2);
    assert_eq!(
        Pin::new(&mut later).poll(&mut cx),
        Poll::Ready(Err(CaptureClosed))
    );
}

#[test]
fn unwinding_the_window_owner_closes_armed_frames_and_pending_subscriptions() {
    let (session, mut sink) = CaptureSession::open();
    session.arm_first();
    store(&mut *sink, 1);
    let pending = session.after(1);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _window_endpoint = sink;
        panic!("window owner cancelled");
    }));
    assert!(result.is_err());
    assert!(session.latest().is_none());
    assert!(session.take_first().is_none());
    assert_eq!(pending.now_or_never(), Some(Err(CaptureClosed)));
    session.arm_first();
    assert!(!session.0.borrow().armed);
}
