//! Mode orchestration: step effect time before each measured sample.
use super::*;

/// Room for one more whole wheel step down on the root scroller.
fn scroll_room(cx: &AsyncApp, window: WindowHandle<View>) -> Result<bool> {
    Ok(window.read_with(cx, |view, _| {
        match (view.doc.root_scroll_offset(), view.doc.root_scroll_max()) {
            (Some(at), Some(max)) => max.1 - at.1 >= crate::frames::WHEEL_STEP - 0.5,
            _ => false,
        }
    })?)
}

/// The modes of `args` on its input: cold load of the input, then each phase.
pub(crate) async fn measure(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    capture: &CaptureSession,
    luid: &work::Adapter,
) -> Result<(serde_json::Map<String, serde_json::Value>, Frames)> {
    super::super::animation::prepare(cx, window, args, 0)?;
    let mut modes = serde_json::Map::new();
    // Work samples of each mode stay numbers until the report is printed.
    let mut work_samples = Vec::with_capacity(3);
    let mut frames = Frames::new(args)?;
    let mut snapshot_pending = args.snapshot.as_deref();
    if matches!(
        args.mode,
        Mode::Warm | Mode::Relayout | Mode::Scroll | Mode::Edit
    ) {
        sample(cx, window, fonts, args, Step::Cold(None), capture, luid).await?;
    }
    if args.mode == Mode::Edit {
        let (edits, work) = crate::edits::run(cx, window, fonts, args, capture, luid).await?;
        modes.insert("edit".into(), edits);
        work_samples.push(("edit", work));
    }
    for &(name, cold) in args.mode.phases() {
        let mut samples = Vec::with_capacity(args.runs);
        let mut diagnostics = Vec::with_capacity(args.runs);
        let mut work = Vec::with_capacity(args.runs);
        // Scroll's load is a cold step too: its steps start after the same quiet window.
        let (step, quiet) = if args.mode == Mode::Scroll {
            (
                Step::Warm,
                Some(crate::memory::before_phase(cx, capture, args).await?),
            )
        } else {
            crate::memory::phase_step(cx, capture, args, cold).await?
        };
        let series = crate::series::Series::new(args);
        let series_start = Instant::now();
        let mut index = 0;
        let mut totals = Vec::with_capacity(args.runs);
        while series.more(index, series_start.elapsed(), &totals) {
            // A timed scroll series ends where the document does: the next wheel step
            // could no longer move the root by a whole step.
            if series.timed() && args.mode == Mode::Scroll && !scroll_room(cx, window)? {
                break;
            }
            index += 1;
            super::super::animation::prepare(cx, window, args, index)?;
            frames.prepare(cx, window, index)?;
            let (sample, frame, counts) =
                sample(cx, window, fonts, args, step, capture, luid).await?;
            let kept = crate::frames::keeps_step(args.mode == Mode::Scroll, index);
            if kept {
                work.push(counts);
            }
            if let Some(path) = snapshot_pending.take() {
                pixels::save(&frame.pixels, path)?;
            }
            frames.observe(cx, window, frame, name, index)?;
            frames
                .settle((&mut *cx, window, fonts, args, capture, luid), step)
                .await?;
            if kept {
                totals.extend(crate::series::total(&sample.phases));
                samples.push(sample.phases);
                diagnostics.push(sample.diagnostics);
            }
        }
        if samples.len() < series.min_frames() {
            return Err(format!(
                "{name}: only {} whole wheel steps fit in the document (at least {} needed)",
                samples.len(),
                series.min_frames()
            )
            .into());
        }
        #[cfg_attr(not(feature = "heap-report"), allow(unused_mut))]
        let mut mode = report::measured_mode(&samples, &diagnostics);
        #[cfg(feature = "heap-report")]
        crate::holders::attach(cx, window, name, &mut mode)?;
        if let Some(quiet) = &quiet {
            mode["start_busy_steady_windows"] = quiet["busy_windows"].clone();
            mode["start_quiet"] = crate::memory::quiet_summary(quiet);
        }
        modes.insert(name.into(), mode);
        work_samples.push((name, work));
    }
    for (name, samples) in &work_samples {
        if let Some(mode) = modes.get_mut(*name) {
            report::attach_work(mode, work::json(samples));
        }
    }
    Ok((modes, frames))
}
