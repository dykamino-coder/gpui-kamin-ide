//! KaminIDE patch: read the rendered frame back from the GPU, for the WPT runner.
//!
//! The runner used to capture its window with `PrintWindow`, i.e. from the DWM
//! compositor: an occluded or off-screen window gets its frames consumed rarely,
//! the swap chain's latency gate then skips drawing, and captures came late or
//! blank. With
//! capture enabled the renderer copies every drawn frame's back buffer to memory
//! before presenting and no longer waits for the compositor, so the result does
//! not depend on where the window is or what covers it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);
static SEQ: AtomicU64 = AtomicU64::new(0);
static FRAMES: Mutex<Frames> = Mutex::new(Frames { latest: None, first: None });

struct Frames {
    latest: Option<CapturedFrame>,
    // Some(None) arms the next GPU readback; later frames cannot replace it.
    first: Option<Option<CapturedFrame>>,
}

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

#[doc(hidden)]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// The most recently drawn frame, if any.
pub fn latest() -> Option<CapturedFrame> {
    FRAMES.lock().ok()?.latest.clone()
}

/// Retain the next drawn frame until `take_first`, even if more frames arrive.
/// Arm on the UI thread after replacing the document and before requesting redraw.
pub fn arm_first() {
    if let Ok(mut frames) = FRAMES.lock() {
        frames.first = Some(None);
    }
}

/// Consume the armed frame, or keep waiting when no GPU draw has completed.
pub fn take_first() -> Option<CapturedFrame> {
    let mut frames = FRAMES.lock().ok()?;
    let frame = frames.first.as_mut()?.take()?;
    frames.first = None;
    Some(frame)
}

#[doc(hidden)]
pub fn store(width: u32, height: u32, bgra: Vec<u8>) {
    let seq = SEQ.fetch_add(1, Ordering::AcqRel) + 1;
    if let Ok(mut frames) = FRAMES.lock() {
        let frame = CapturedFrame { width, height, bgra, seq };
        if let Some(first @ None) = &mut frames.first {
            *first = Some(frame.clone());
        }
        frames.latest = Some(frame);
    }
}
