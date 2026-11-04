//! Дедлайн readback отменяет регистрацию таймера сразу после завершения кадра.

use futures::future::{Either, select};
use std::{future::Future, time::Duration};

pub(super) async fn within<T>(
    event: impl Future<Output = T> + Unpin,
    duration: Duration,
) -> Option<T> {
    // GPUI timer оставляет runnable в очереди Windows до дедлайна даже при drop.
    // Этот таймер удаляет регистрацию и Waker, когда проигрывает select.
    match select(event, async_io::Timer::after(duration)).await {
        Either::Left((value, _)) => Some(value),
        Either::Right(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        pin::pin,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };

    struct Observer;
    impl Wake for Observer {
        fn wake(self: Arc<Self>) {}
    }

    #[test]
    fn completed_event_wins_and_missing_event_times_out() {
        assert_eq!(
            async_io::block_on(within(futures::future::ready(7), Duration::from_secs(60))),
            Some(7)
        );
        assert_eq!(
            async_io::block_on(within(futures::future::pending::<u64>(), Duration::ZERO)),
            None
        );
    }

    #[test]
    fn repeated_completed_frames_release_pending_deadline_wakers() {
        let observer = Arc::new(Observer);
        let waker = Waker::from(observer.clone());
        let mut cx = Context::from_waker(&waker);
        for _ in 0..1000 {
            let mut ready = false;
            let event = futures::future::poll_fn(|_| {
                if std::mem::replace(&mut ready, true) {
                    Poll::Ready(7)
                } else {
                    Poll::Pending
                }
            });
            let mut wait = pin!(within(event, Duration::from_secs(60)));
            assert!(wait.as_mut().poll(&mut cx).is_pending());
            assert_eq!(wait.as_mut().poll(&mut cx), Poll::Ready(Some(7)));
        }
        // Reactor обрабатывает удаления до ближайшего таймера, а не через 60 секунд.
        async_io::block_on(async_io::Timer::after(Duration::from_millis(20)));
        drop(waker);
        assert_eq!(Arc::strong_count(&observer), 1);
    }
}
