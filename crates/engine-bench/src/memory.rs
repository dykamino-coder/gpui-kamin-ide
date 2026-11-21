//! Устойчивая память страницы и освобождение документа без принудительной очистки.
//!
//! A quiet window is an interval without frames. A page may still paint after
//! its load (late image decodes, fonts, animations), so the window does not
//! refuse on the first frame: it starts again up to [`QUIET_WINDOWS`] times,
//! and a page that never goes quiet is measured over the last full window
//! with a note (`quiet: false`, frame count). The measured frames themselves still
//! refuse an extra frame (`run::sample`), so real engine faults are not
//! hidden. Window memory is the median of the samples over the window, not one point.

use super::{args::Args, process::Snapshot, run, view::View};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::{BROWSER_CSS, Document, host::capture::CaptureSession, metrics::FontContext};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// How many windows to wait for quiet before a busy page is measured with a note.
pub(super) const QUIET_WINDOWS: u32 = 10;
/// Step of memory sampling and frame checks inside a window.
const TICK_MS: u64 = 25;

/// Окно устойчивой памяти между построением документа и замерами того же
/// документа (warm, relayout, edit). Без него память этих кадров зависит от
/// времени холодных кадров, а не от страницы: отложенная очистка mimalloc
/// (около 1 с) успевает сработать до них у медленного процесса и не успевает
/// у быстрого, и быстрый кандидат выглядит тяжелее базы.
pub(super) async fn steady(cx: &AsyncApp, capture: &CaptureSession) -> Result<Value> {
    settle(
        cx,
        capture,
        super::args::MEMORY_STEADY_WINDOW_MS,
        QUIET_WINDOWS,
    )
    .await
}

/// Quiet window of a plan (`Args::steady_each_phase` = false): no frames, as
/// [`steady`], but without waiting for the delayed memory purge: a plan measures memory
/// once (`plan.rs`), not before every phase.
const QUIET_WINDOW_MS: u64 = 100;

/// Window before a phase of the same document: steady memory or the plan's quiet window.
pub(super) async fn before_phase(
    cx: &AsyncApp,
    capture: &CaptureSession,
    args: &Args,
) -> Result<Value> {
    if args.steady_each_phase {
        return steady(cx, capture).await;
    }
    settle(cx, capture, QUIET_WINDOW_MS, QUIET_WINDOWS).await
}

/// Frame step of the measured phase and the quiet before it (`None` for a cold phase).
pub(super) async fn phase_step(
    cx: &AsyncApp,
    capture: &CaptureSession,
    args: &Args,
    cold: bool,
) -> Result<(run::Step<'static>, Option<Value>)> {
    if cold {
        return Ok((run::Step::Cold(None), None));
    }
    let quiet = before_phase(cx, capture, args).await?;
    Ok((run::Step::Warm, Some(quiet)))
}

/// Steady memory of the open plan page: an idle window, without an empty document.
pub(super) async fn page_audit(
    cx: &AsyncApp,
    capture: &CaptureSession,
    milliseconds: u64,
) -> Result<Value> {
    settle(cx, capture, milliseconds, QUIET_WINDOWS).await
}

/// Short quiet summary for the phase report.
pub(super) fn quiet_summary(window: &Value) -> Value {
    json!({"quiet": window["quiet"], "busy_windows": window["busy_windows"],
        "frames_in_measured_window": window["unexpected_frames"]})
}

/// Waits for a window without frames up to `tries` times; a window ends at its first frame.
/// A page that never went quiet is measured over one more full window with `quiet: false`.
async fn settle(
    cx: &AsyncApp,
    capture: &CaptureSession,
    milliseconds: u64,
    tries: u32,
) -> Result<Value> {
    let started = Instant::now();
    for busy in 0..tries {
        let (mut value, frames) = window(cx, capture, milliseconds, true).await?;
        if frames == 0 {
            mark(&mut value, busy, true, started);
            return Ok(value);
        }
    }
    let (mut value, frames) = window(cx, capture, milliseconds, false).await?;
    mark(&mut value, tries, frames == 0, started);
    Ok(value)
}

fn mark(value: &mut Value, busy: u32, quiet: bool, started: Instant) {
    value["busy_windows"] = json!(busy);
    value["quiet"] = json!(quiet);
    value["settle_ms"] = json!(started.elapsed().as_secs_f64() * 1000.);
}

/// One window: memory at its start, at its end and the sample median; frames in the window.
async fn window(
    cx: &AsyncApp,
    capture: &CaptureSession,
    milliseconds: u64,
    stop_on_frame: bool,
) -> Result<(Value, u64)> {
    let sequence = capture.sequence();
    let before = Snapshot::read()?;
    let started = Instant::now();
    let length = Duration::from_millis(milliseconds);
    let mut rss = Vec::with_capacity((milliseconds / TICK_MS + 1) as usize);
    let mut commit = Vec::with_capacity(rss.capacity());
    while let Some(left) = length
        .checked_sub(started.elapsed())
        .filter(|d| !d.is_zero())
    {
        cx.background_executor()
            .timer(left.min(Duration::from_millis(TICK_MS)))
            .await;
        let sample = Snapshot::read()?;
        rss.push(sample.rss);
        commit.push(sample.commit);
        if stop_on_frame && capture.sequence() != sequence {
            break;
        }
    }
    let after = Snapshot::read()?;
    let frames = capture.sequence().wrapping_sub(sequence);
    Ok((
        json!({"start": before.json(), "end": after.json(),
        "median": {"rss_bytes": median(&mut rss), "private_commit_bytes": median(&mut commit),
            "samples": rss.len(), "tick_ms": TICK_MS},
        "idle_ms": started.elapsed().as_secs_f64() * 1000.,
        "cpu_ms": after.cpu_100ns.saturating_sub(before.cpu_100ns) as f64 / 10_000.,
        "unexpected_frames": frames}),
        frames,
    ))
}

/// Median (the lower one for an even count); `None` for no samples.
fn median(values: &mut [u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[(values.len() - 1) / 2])
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn finish(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    capture: &CaptureSession,
    luid: &super::work::Adapter,
) -> Result<Value> {
    let page = settle(cx, capture, args.memory_settle_ms, QUIET_WINDOWS).await?;
    window.update(cx, |view, _, _| -> Result<()> {
        view.doc = Document::builder("", BROWSER_CSS).fonts(fonts).build()?;
        Ok(())
    })??;
    // GPUI держит предыдущий и текущий кадры. Два пустых кадра сменяют оба,
    // сохраняя тот же HWND, GPU-устройство и системные кэши процесса.
    for _ in 0..2 {
        run::sample(cx, window, fonts, args, run::Step::Warm, capture, luid).await?;
    }
    let empty = settle(cx, capture, args.memory_settle_ms, QUIET_WINDOWS).await?;
    Ok(json!({"stable_warm_frames": args.runs, "page": page,
        "after_empty_document_two_frames": empty,
        "scope": "live window; only the host's own idle memory release, no harness collection or working-set trimming"}))
}

#[cfg(test)]
mod tests {
    use super::median;

    #[test]
    fn median_takes_the_middle_sample_and_the_lower_of_two() {
        assert_eq!(median(&mut []), None);
        assert_eq!(median(&mut [7]), Some(7));
        assert_eq!(median(&mut [9, 1, 5]), Some(5));
        assert_eq!(median(&mut [4, 1, 3, 2]), Some(2));
    }
}
