//! Сессия одного окна: завершения readback, подписки и выбор first/latest.

mod completion;
#[cfg(test)]
mod tests;

pub use completion::{CaptureClosed, Completion};
use gpui::frame_capture::{CapturedFrame, FrameCaptureSink};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

/// Кадр с номером завершения внутри этой сессии.
#[derive(Clone)]
pub struct Frame {
    pub seq: u64,
    pub pixels: CapturedFrame,
    /// Когда пиксели этого кадра дошли до сессии (в памяти CPU).
    pub delivered: Instant,
}

#[derive(Default)]
struct State {
    closed: bool,
    /// Номер последнего доставленного (или потерянного) кадра.
    seq: u64,
    /// Номер последнего отправленного на асинхронный readback кадра.
    submitted: u64,
    /// `first` берётся только из кадров, отправленных после `arm_first`.
    armed_after: u64,
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
        let endpoint = Endpoint(session.0.clone());
        (session, Box::new(endpoint))
    }

    /// Граница запроса: все кадры, уже отправленные или доставленные.
    /// `after(sequence())` ждёт кадр, отправленный позже, даже если более
    /// ранний ещё в пути (асинхронный readback доставляет их по порядку).
    pub fn sequence(&self) -> u64 {
        self.0.borrow().fence()
    }

    pub fn readback_time(&self) -> Duration {
        Duration::from_micros(self.0.borrow().readback_us)
    }

    pub fn latest(&self) -> Option<Frame> {
        self.0.borrow().latest.clone()
    }

    /// Передать буфер единственному потребителю без копии, сохранив fence
    /// и независимый first. Старый кадр остаётся доступен другим читателям.
    pub fn take_latest_after(&self, seq: u64) -> Option<Frame> {
        let mut state = self.0.borrow_mut();
        if state.latest.as_ref()?.seq <= seq {
            return None;
        }
        state.latest.take()
    }

    pub fn arm_first(&self) {
        let mut state = self.0.borrow_mut();
        state.first = None;
        state.armed = !state.closed;
        state.armed_after = state.fence();
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

impl State {
    fn fence(&self) -> u64 {
        self.seq.max(self.submitted)
    }
}

// Endpoint владеет состоянием окна напрямую; потребительская сессия ему не нужна.
struct Endpoint(Rc<RefCell<State>>);

impl Endpoint {
    /// Закрыть очередной номер: с пикселями либо как потерянный кадр.
    fn finish(&mut self, pixels: Option<(CapturedFrame, Duration)>) {
        let mut state = self.0.borrow_mut();
        state.seq += 1;
        let seq = state.seq;
        if let Some((pixels, readback)) = pixels {
            state.readback_us += readback.as_micros() as u64;
            let delivered = Instant::now();
            let frame = Frame {
                seq,
                pixels,
                delivered,
            };
            if state.armed && seq > state.armed_after {
                state.first = Some(frame.clone());
                state.armed = false;
            }
            state.latest = Some(frame);
        }
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

impl FrameCaptureSink for Endpoint {
    fn completed(&mut self, pixels: CapturedFrame, readback: Duration) {
        self.finish(Some((pixels, readback)));
    }

    fn submitted(&mut self) {
        let mut state = self.0.borrow_mut();
        state.submitted = state.fence() + 1;
    }

    fn discarded(&mut self) {
        self.finish(None);
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
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
