//! KaminIDE patch: read the rendered frame back from the GPU, for the WPT runner.
//!
//! The runner used to capture its window with `PrintWindow`, i.e. from the DWM
//! compositor: an occluded or off-screen window gets its frames consumed rarely,
//! the swap chain's latency gate then skips drawing, and captures came late or
//! blank (2026-10-06: 11 instead of 70 pairs/min, 350 false blank pages). With
//! capture enabled the renderer copies every drawn frame's back buffer to memory
//! before presenting and no longer waits for the compositor, so the result does
//! not depend on where the window is or what covers it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);
static SEQ: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<CapturedFrame>> = Mutex::new(None);

/// One captured frame: BGRA rows top to bottom, `width * 4` bytes each.
#[derive(Clone)]
pub struct CapturedFrame {
    /// Width in device pixels.
    pub width: u32,
    /// Height in device pixels.
    pub height: u32,
    /// Pixel bytes, B G R A.
    pub bgra: Vec<u8>,
    /// Increases with every captured frame.
    pub seq: u64,
}

/// Start capturing every drawn frame (process-wide; meant for a single window).
pub fn enable() {
    ENABLED.store(true, Ordering::Release);
}

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// The most recently drawn frame, if any.
pub fn latest() -> Option<CapturedFrame> {
    LAST.lock().ok()?.clone()
}

#[allow(dead_code)]
pub(crate) fn store(width: u32, height: u32, bgra: Vec<u8>) {
    let seq = SEQ.fetch_add(1, Ordering::AcqRel) + 1;
    if let Ok(mut last) = LAST.lock() {
        *last = Some(CapturedFrame { width, height, bgra, seq });
    }
}
