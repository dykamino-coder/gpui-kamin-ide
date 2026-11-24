//! Сессия одного окна: завершения readback, подписки и выбор first/latest.

mod completion;
#[cfg(test)]
mod tests;

pub use completion::{CaptureClosed, Completion};
use gpui::frame_capture::{CapturedFrame, FrameCaptureSink, RasterReport};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

/// Кадр с номером завершения внутри этой сессии. Буфер readback один на кадр:
/// first и latest делят его (как результат `CopyOutputRequest` в Chromium
/// передаётся владельцу без копии пикселей).
#[derive(Clone)]
pub struct Frame {
    pub seq: u64,
    pub pixels: Rc<CapturedFrame>,
    /// Момент завершения readback в потоке окна, до пробуждения ждущих.
    pub completed_at: Instant,
}

impl Frame {
    /// Пиксели во владение: без копии, если кадр больше никто не держит.
    pub fn into_pixels(self) -> CapturedFrame {
        Rc::try_unwrap(self.pixels).unwrap_or_else(|shared| (*shared).clone())
    }
}

#[derive(Default)]
struct State {
    closed: bool,
    seq: u64,
    readback_us: u64,
    copy_us: u64,
    /// What the last captured frame rasterized and its GPU wait (`--why`).
    raster: Option<RasterReport>,
    wait_us: u64,
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

    pub fn sequence(&self) -> u64 {
        self.0.borrow().seq
    }

    pub fn readback_time(&self) -> Duration {
        Duration::from_micros(self.0.borrow().readback_us)
    }

    /// Часть `readback_time` после готовности кадра на GPU: копия цели в память.
    pub fn copy_time(&self) -> Duration {
        Duration::from_micros(self.0.borrow().copy_us)
    }

    /// The raster of the last captured frame, when the backend reports it.
    pub fn raster_report(&self) -> Option<RasterReport> {
        self.0.borrow().raster.clone()
    }

    /// The part of `readback_time` before the frame is complete: the wait for
    /// the GPU to execute it (on a software adapter, its CPU raster).
    pub fn gpu_wait_time(&self) -> Duration {
        Duration::from_micros(self.0.borrow().wait_us)
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

// Endpoint владеет состоянием окна напрямую; потребительская сессия ему не нужна.
struct Endpoint(Rc<RefCell<State>>);

impl FrameCaptureSink for Endpoint {
    fn completed(&mut self, pixels: CapturedFrame, readback: Duration) {
        let mut state = self.0.borrow_mut();
        state.seq += 1;
        state.readback_us += readback.as_micros() as u64;
        let seq = state.seq;
        let frame = Frame {
            seq,
            pixels: Rc::new(pixels),
            completed_at: Instant::now(),
        };
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

    fn copied(&mut self, copy: Duration) {
        self.0.borrow_mut().copy_us += copy.as_micros() as u64;
    }

    fn raster(&mut self, report: RasterReport) {
        self.0.borrow_mut().raster = Some(report);
    }

    fn waited(&mut self, wait: Duration) {
        self.0.borrow_mut().wait_us += wait.as_micros() as u64;
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
