//! KaminIDE patch (BR-31A): presenting without blocking the UI thread.
//!
//! The frame-latency gate (#76) keeps one frame in the compositor queue and
//! drops a frame instead of waiting for it. `Present` itself must not wait
//! either: while the GPU or the compositor was behind, a plain `Present`
//! blocked the UI thread for about 1 s per frame, and webview pulls, which
//! run between frames, waited behind it. A dropped frame is presented by the
//! next paint (`DirectXRenderer::needs_presentation`).

use std::cell::Cell;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_WAS_STILL_DRAWING;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::HRESULT;

/// The waitable object of the swap chain (latency 1); closed when the
/// resources are recreated (device lost, resize to a new swap chain).
pub(super) struct FrameLatencyGate {
    handle: HANDLE,
    /// A count taken from the gate whose frame has not been queued yet. A
    /// dropped `Present` keeps it: the gate is signaled only when a queued
    /// frame retires, so waiting again would lose the count for good and
    /// close the gate.
    held: Cell<bool>,
}

impl FrameLatencyGate {
    pub(super) fn new(handle: HANDLE) -> Self {
        Self {
            handle,
            held: Cell::new(false),
        }
    }

    /// True when a frame may be queued now; never waits.
    pub(super) fn try_take(&self) -> bool {
        if self.held.get() {
            return true;
        }
        let taken = unsafe { WaitForSingleObject(self.handle, 0) } == WAIT_OBJECT_0;
        self.held.set(taken);
        taken
    }

    /// The frame of the held count was queued.
    pub(super) fn spend(&self) {
        self.held.set(false);
    }
}

impl Drop for FrameLatencyGate {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Whether a `Present(.., DXGI_PRESENT_DO_NOT_WAIT)` result queued the frame.
pub(super) fn frame_shown(result: HRESULT) -> Result<bool> {
    if result == DXGI_ERROR_WAS_STILL_DRAWING {
        return Ok(false);
    }
    result.ok().context("Presenting swap chain failed")?;
    Ok(true)
}

pub(super) fn count_present_call(spent: Duration) {
    let us = u32::try_from(spent.as_micros()).unwrap_or(u32::MAX);
    gpui::frame_perf::SWAP_US.fetch_add(us, Relaxed);
}

pub(super) fn count_dropped_frame() {
    gpui::frame_perf::DROPPED_FRAMES.fetch_add(1, Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::S_OK;
    use windows::Win32::Graphics::Dxgi::DXGI_ERROR_DEVICE_REMOVED;
    use windows::Win32::System::Threading::{CreateSemaphoreW, ReleaseSemaphore};

    fn gate(count: i32) -> FrameLatencyGate {
        FrameLatencyGate::new(unsafe { CreateSemaphoreW(None, count, 1, None) }.unwrap())
    }

    #[test]
    fn a_present_that_would_wait_drops_the_frame() {
        assert!(!frame_shown(DXGI_ERROR_WAS_STILL_DRAWING).unwrap());
        assert!(frame_shown(S_OK).unwrap());
        // DXGI_STATUS_OCCLUDED is a success code: the frame was queued.
        assert!(frame_shown(HRESULT(0x087A_0001)).unwrap());
        assert!(frame_shown(DXGI_ERROR_DEVICE_REMOVED).is_err());
    }

    #[test]
    fn a_dropped_frame_keeps_its_gate_count() {
        let g = gate(1);
        assert!(g.try_take());
        // Present dropped the frame: no spend, so the next attempt may queue
        // although nothing will signal the gate for this frame.
        assert!(g.try_take());
        g.spend();
        assert!(!g.try_take(), "queued frame has not retired yet");
        unsafe { ReleaseSemaphore(g.handle, 1, None) }.unwrap();
        assert!(g.try_take());
    }
}
