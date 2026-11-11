//! Устойчивая память страницы и освобождение документа без принудительной очистки.

use super::{args::Args, process::Snapshot, run, view::View};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::{BROWSER_CSS, Document, host::capture::CaptureSession, metrics::FontContext};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Окно устойчивой памяти между построением документа и замерами того же
/// документа (warm, relayout, edit). Без него память этих кадров зависит от
/// времени холодных кадров, а не от страницы: отложенная очистка mimalloc
/// (около 1 с) успевает сработать до них у медленного процесса и не успевает
/// у быстрого, и быстрый кандидат выглядит тяжелее базы.
pub(super) async fn steady(cx: &AsyncApp, capture: &CaptureSession) -> Result<()> {
    idle(cx, capture, super::args::MEMORY_STEADY_WINDOW_MS)
        .await
        .map(drop)
}

/// Шаг кадров фазы замера; фаза того же документа начинается после [`steady`].
pub(super) async fn phase_step(
    cx: &AsyncApp,
    capture: &CaptureSession,
    cold: bool,
) -> Result<run::Step<'static>> {
    if cold {
        return Ok(run::Step::Cold(None));
    }
    steady(cx, capture).await?;
    Ok(run::Step::Warm)
}

async fn idle(cx: &AsyncApp, capture: &CaptureSession, milliseconds: u64) -> Result<Value> {
    let sequence = capture.sequence();
    let before = Snapshot::read()?;
    let started = Instant::now();
    cx.background_executor()
        .timer(Duration::from_millis(milliseconds))
        .await;
    let after = Snapshot::read()?;
    if capture.sequence() != sequence {
        return Err("memory audit received an unexpected frame during its idle interval".into());
    }
    Ok(json!({"start": before.json(), "end": after.json(),
        "idle_ms": started.elapsed().as_secs_f64() * 1000.,
        "cpu_ms": after.cpu_100ns.saturating_sub(before.cpu_100ns) as f64 / 10_000.,
        "unexpected_frames": 0}))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn finish(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    capture: &CaptureSession,
    luid: (i32, u32),
) -> Result<Value> {
    let page = idle(cx, capture, args.memory_settle_ms).await?;
    window.update(cx, |view, _, _| -> Result<()> {
        view.doc = Document::builder("", BROWSER_CSS).fonts(fonts).build()?;
        Ok(())
    })??;
    // GPUI держит предыдущий и текущий кадры. Два пустых кадра сменяют оба,
    // сохраняя тот же HWND, GPU-устройство и системные кэши процесса.
    for _ in 0..2 {
        run::sample(cx, window, fonts, args, run::Step::Warm, capture, luid).await?;
    }
    let empty = idle(cx, capture, args.memory_settle_ms).await?;
    Ok(json!({"stable_warm_frames": args.runs, "page": page,
        "after_empty_document_two_frames": empty,
        "scope": "live window; only the host's own idle memory release, no harness collection or working-set trimming"}))
}
