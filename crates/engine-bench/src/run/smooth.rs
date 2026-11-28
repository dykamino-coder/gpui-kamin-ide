//! Live-vsync trajectory journal; no single-frame timing estimate is inferred.
use super::*;
use std::time::Duration;

pub(super) async fn run(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    args: &Args,
    capture: &CaptureSession,
) -> Result<serde_json::Value> {
    let quiet = crate::memory::before_phase(cx, capture, args).await?;
    let mut paths = Vec::with_capacity(args.runs);
    for index in 0..args.runs {
        let watch = crate::why::Watch::begin(args, cx, window)?;
        let (from, target) = window.update(cx, |view, window, cx| -> Result<_> {
            let from = view
                .doc
                .root_scroll_offset()
                .ok_or("missing root scrollport")?;
            let max = view
                .doc
                .root_scroll_max()
                .ok_or("missing root scroll range")?;
            let y = if index.is_multiple_of(2) {
                (from.1 + args.scroll_step).min(max.1)
            } else {
                (from.1 - args.scroll_step).max(0.)
            };
            if (y - from.1).abs() < 0.5 {
                return Err("smooth trajectory has no scroll room".into());
            }
            view.doc
                .scroll_to(from.0, y, kamin_html::ScrollBehavior::Smooth, window, cx);
            Ok((from, (from.0, y)))
        })??;
        let started = Instant::now();
        let error = loop {
            cx.background_executor()
                .timer(Duration::from_millis(8))
                .await;
            let at = window.read_with(cx, |view, _| view.doc.root_scroll_offset())?;
            if at == Some(target) {
                break None;
            }
            if started.elapsed() >= Duration::from_secs(5) {
                break Some(format!(
                    "smooth trajectory timed out at {at:?}, wanted {target:?}"
                ));
            }
        };
        if let Some(watch) = watch {
            watch.end(args, cx, window, Step::Warm, None, None, error.clone())?;
        }
        if let Some(error) = error {
            return Err(error.into());
        }
        paths.push(json!({"from": from, "to": target}));
    }
    Ok(
        json!({"trajectories": paths, "clock": "live vsync", "start_quiet": quiet,
        "timing": "not sampled: whole trajectories; animation work is in --why"}),
    )
}
