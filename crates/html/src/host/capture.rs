//! Сессия одного окна: завершения readback, подписки и выбор first/latest.

mod completion;
#[cfg(test)]
mod tests;

pub use completion::{CaptureClosed, Completion};
use gpui::frame_capture::{CapturedFrame, FrameCaptureSink};
use std::{cell::RefCell, rc::Rc, time::Duration};

/// Кадр с номером завершения внутри этой сессии.
#[derive(Clone)]
pub struct Frame {
    pub seq: u64,
    pub pixels: CapturedFrame,
}

#[derive(Default)]
struct State {
    closed: bool,
    seq: u64,
    readback_us: u64,
    latest: Option<Frame>,
    first: Option<Frame>,
    armed: bool,
    waiters: Vec<completion::Waiter>,
}

/// Клонирование разделяет сессию, но не создаёт второго производителя кадров.
#[derive(Clone)]
pub struct CaptureSession(Rc<RefCell<State>>);

impl CaptureSession {
    /// Endpoint перемещается ровно в одно окно; его drop закрывает подписки.
    pub fn open() -> (Self, Box<dyn FrameCaptureSink>) {
        let session = Self(Rc::default());
        (session.clone(), Box::new(Endpoint(session)))
    }

    pub fn sequence(&self) -> u64 {
        self.0.borrow().seq
    }

    pub fn readback_time(&self) -> Duration {
        Duration::from_micros(self.0.borrow().readback_us)
    }

    pub fn latest(&self) -> Option<Frame> {
        self.0.borrow().latest.clone()
    }

    pub fn arm_first(&self) {
        let mut state = self.0.borrow_mut();
        state.first = None;
        state.armed = !state.closed;
    }

    pub fn take_first(&self) -> Option<Frame> {
        self.0.borrow_mut().first.take()
    }

    pub fn after(&self, seq: u64) -> Completion {
        let mut state = self.0.borrow_mut();
        let result = if state.closed {
            Some(Err(CaptureClosed))
        } else {
            (state.seq > seq).then_some(Ok(state.seq))
        };
        let (event, waiter) = Completion::new(seq, result);
        state.waiters.retain(|waiter| waiter.alive());
        if result.is_none() {
            state.waiters.push(waiter);
        }
        event
    }
}

struct Endpoint(CaptureSession);

impl FrameCaptureSink for Endpoint {
    fn completed(&mut self, pixels: CapturedFrame, readback: Duration) {
        let mut state = self.0.0.borrow_mut();
        state.seq += 1;
        state.readback_us += readback.as_micros() as u64;
        let seq = state.seq;
        let frame = Frame { seq, pixels };
        if state.armed {
            state.first = Some(frame.clone());
            state.armed = false;
        }
        state.latest = Some(frame);
        let mut wakes = Vec::new();
        state
            .waiters
            .retain(|waiter| waiter.complete(Ok(seq), &mut wakes));
        drop(state);
        for waker in wakes {
            waker.wake();
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        let mut state = self.0.0.borrow_mut();
        state.closed = true;
        state.latest = None;
        state.first = None;
        let mut wakes = Vec::new();
        for waiter in state.waiters.drain(..) {
            waiter.complete(Err(CaptureClosed), &mut wakes);
        }
        drop(state);
        for waker in wakes {
            waker.wake();
        }
    }
}
