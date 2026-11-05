//! Числовые адаптеры используют общий алгоритм без приостановок и обхода DOM.
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};
pub(super) fn run<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous fragment probes never suspend"),
    }
}
