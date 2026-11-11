//! KaminIDE patch: `Platform::on_idle` on a thread timer of the main thread.
//!
//! Nothing here takes heap memory: the callback is a plain `fn`, the state is
//! a few atomics, and the timer is the system's (`SetTimer` with a timer
//! procedure, dispatched by the message loop). The first frame after an idle
//! period arms the timer for `delay`; frames drawn while it waits only store
//! their time, and the timer re-arms itself for the rest of the period, so
//! the callback runs once, `delay` after the last frame, and an idle app has
//! no timer at all.

use std::sync::{
    OnceLock,
    atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
};
use std::time::Duration;
use windows::Win32::{
    Foundation::HWND,
    System::SystemInformation::GetTickCount64,
    UI::WindowsAndMessaging::{KillTimer, SetTimer},
};

/// The registered period (ms) and callback; the first registration holds.
static IDLE: OnceLock<(u64, fn())> = OnceLock::new();
/// `GetTickCount64` of the last drawn frame.
static LAST_FRAME: AtomicU64 = AtomicU64::new(0);
/// Id of the armed thread timer, 0 when none is armed.
static TIMER: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn on_idle(delay: Duration, callback: fn()) {
    let ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX).max(1);
    _ = IDLE.set((ms, callback));
}

pub(crate) fn frame_drawn() {
    let Some(&(delay, _)) = IDLE.get() else {
        return;
    };
    LAST_FRAME.store(unsafe { GetTickCount64() }, Relaxed);
    if TIMER.load(Relaxed) == 0 {
        arm(0, delay);
    }
}

/// (Re)arm the thread timer `id` (0: a new one) to fire in `ms`.
fn arm(id: usize, ms: u64) {
    let ms = u32::try_from(ms).unwrap_or(u32::MAX);
    // SAFETY: a thread timer of the calling (main) thread; `fire` matches TIMERPROC.
    let timer = unsafe { SetTimer(None, id, ms, Some(fire)) };
    TIMER.store(timer, Relaxed);
}

/// What is left of the idle period after `idle` ms of it: `None` once it is over.
fn remaining(idle: u64, delay: u64) -> Option<u64> {
    (idle < delay).then(|| delay - idle)
}

unsafe extern "system" fn fire(_: HWND, _: u32, id: usize, _: u32) {
    let Some(&(delay, callback)) = IDLE.get() else {
        return;
    };
    let idle = unsafe { GetTickCount64() }.saturating_sub(LAST_FRAME.load(Relaxed));
    match remaining(idle, delay) {
        Some(rest) => arm(id, rest),
        None => {
            // SAFETY: the timer of this thread that is firing.
            _ = unsafe { KillTimer(None, id) };
            TIMER.store(0, Relaxed);
            callback();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::remaining;

    #[test]
    fn the_period_runs_from_the_last_frame() {
        assert_eq!(remaining(0, 1500), Some(1500));
        assert_eq!(remaining(600, 1500), Some(900), "a frame 600 ms ago");
        assert_eq!(remaining(1500, 1500), None);
        assert_eq!(remaining(4000, 1500), None);
    }
}
