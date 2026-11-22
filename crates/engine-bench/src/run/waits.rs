//! Wait for readback or a skipped presentation with bounded timeouts.
use super::*;
use std::time::{Duration, Instant};

pub(super) async fn wait(cx: &AsyncApp, sequence: u64, capture: &CaptureSession) -> Result<u64> {
    use futures::future::{Either, select};
    let event = capture.after(sequence);
    let timeout = cx.background_executor().timer(Duration::from_secs(60));
    match select(event, timeout).await {
        Either::Left((Ok(seq), _)) => Ok(seq),
        _ => Err("no completed GPU readback within 60 seconds".into()),
    }
}

/// Wait for the frame after `sequence`: its readback, or its decision to
/// skip presentation (`gpui::layer_counters::PRESENTS_SKIPPED`), then
/// `None` and the moment it decided.
pub(super) async fn wait_or_skip(
    cx: &AsyncApp,
    sequence: u64,
    capture: &CaptureSession,
    skipped: u64,
) -> Result<std::result::Result<u64, Instant>> {
    use gpui::layer_counters as c;
    use std::sync::atomic::Ordering::Relaxed;
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if capture.sequence() > sequence {
            return wait(cx, sequence, capture).await.map(Ok);
        }
        if c::PRESENTS_SKIPPED.load(Relaxed) > skipped {
            let at = c::LAST_SKIP
                .lock()
                .ok()
                .and_then(|at| *at)
                .ok_or("skip without time")?;
            return Ok(Err(at));
        }
        cx.background_executor()
            .timer(Duration::from_micros(500))
            .await;
    }
    Err("neither a readback nor a skipped presentation within 60 seconds".into())
}
