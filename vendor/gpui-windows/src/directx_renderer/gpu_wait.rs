//! KaminIDE patch: wait until the GPU has executed every command submitted so
//! far, for as long as that work takes and no longer.
//!
//! A device fence (D3D11.3, `ID3D11Fence`) is signalled on the context after
//! the frame's commands; the kernel sets an event when the GPU passes it, and
//! the thread sleeps on that event. Polling an event query instead spins a
//! core while the driver or the WARP rasterizer threads need it, and a poll
//! that does not flush (`D3D11_ASYNC_GETDATA_DONOTFLUSH`) sees the query
//! complete only once something else submits the queue: on two cores that
//! cost a scheduler quantum (~15.6 ms) per captured frame. Chromium waits for
//! the GPU the same way (`gl::GLFenceWin`, `D3DSharedFence`: a D3D11 fence and
//! `SetEventOnCompletion`). A device without fences polls the query and flushes
//! on every poll, as ANGLE's `Renderer11::finish` does.

use super::*;
use windows::Win32::Foundation::{CloseHandle, HANDLE, S_OK, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::Interface;

/// The longest wait for the GPU before the capture fails (a removed device).
const WAIT_MS: u32 = 10_000;

/// How the device waits for the GPU, found on the first capture.
#[derive(Default)]
pub(super) enum GpuWait {
    #[default]
    Unknown,
    /// A fence with the last value signalled, and the event it sets.
    Fence(ID3D11Fence, u64, OwnedEvent),
    /// No fence on this device: an event query.
    Query,
}

/// An event handle closed on drop.
pub(super) struct OwnedEvent(HANDLE);

impl Drop for OwnedEvent {
    fn drop(&mut self) {
        // SAFETY: the handle came from `CreateEventW` and is closed once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// The device's fence and an auto-reset event, or `None` without fences.
fn fence_of(device: &ID3D11Device) -> Option<GpuWait> {
    let device5: ID3D11Device5 = device.cast().ok()?;
    let mut fence: Option<ID3D11Fence> = None;
    // SAFETY: plain COM calls on a live device.
    unsafe { device5.CreateFence(0, D3D11_FENCE_FLAG_NONE, &mut fence) }.ok()?;
    let event = unsafe { CreateEventW(None, false, false, None) }.ok()?;
    Some(GpuWait::Fence(fence?, 0, OwnedEvent(event)))
}

impl DirectXRenderer {
    /// Block until the GPU has executed every command submitted so far,
    /// without touching the frame's pixels.
    pub(super) fn wait_gpu_idle(&mut self) -> Result<()> {
        let device = self.dev().device.clone();
        let ctx = self.dev().device_context.clone();
        let wait = &mut self.retained.gpu_wait;
        if matches!(wait, GpuWait::Unknown) {
            *wait = fence_of(&device).unwrap_or(GpuWait::Query);
        }
        match wait {
            GpuWait::Fence(fence, value, event) => {
                let ctx4: ID3D11DeviceContext4 = ctx.cast()?;
                *value += 1;
                // SAFETY: COM calls on the live context and fence; the event
                // outlives the wait.
                unsafe {
                    ctx4.Signal(&*fence, *value)?;
                    ctx.Flush();
                    fence.SetEventOnCompletion(*value, event.0)?;
                    if WaitForSingleObject(event.0, WAIT_MS) != WAIT_OBJECT_0 {
                        anyhow::bail!("GPU fence wait timed out");
                    }
                }
                Ok(())
            }
            _ => wait_query(&device, &ctx),
        }
    }
}

/// Poll an event query, flushing the queue on every poll.
fn wait_query(device: &ID3D11Device, ctx: &ID3D11DeviceContext) -> Result<()> {
    let desc = D3D11_QUERY_DESC { Query: D3D11_QUERY_EVENT, MiscFlags: 0 };
    let mut query = None;
    unsafe { device.CreateQuery(&desc, Some(&mut query))? };
    let query: ID3D11Query = query.context("event query missing")?;
    unsafe { ctx.End(&query) };
    let started = std::time::Instant::now();
    loop {
        let mut done = 0i32;
        // The `GetData` wrapper maps S_FALSE ("not yet") to Ok; read the raw HRESULT.
        let hr = unsafe {
            (Interface::vtable(ctx).GetData)(
                Interface::as_raw(ctx),
                Interface::as_raw(&query),
                &mut done as *mut i32 as *mut _,
                std::mem::size_of::<i32>() as u32,
                0,
            )
        };
        hr.ok()?;
        if hr == S_OK && done != 0 {
            return Ok(());
        }
        if started.elapsed().as_millis() > u128::from(WAIT_MS) {
            anyhow::bail!("GPU event query wait timed out");
        }
        std::thread::yield_now();
    }
}
