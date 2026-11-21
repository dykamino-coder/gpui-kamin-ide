//! CPU/GPU samples keep wall-clock timing independent of animation input.
use super::{
    Result, Step,
    waits::{wait, wait_or_skip},
};
use crate::{
    accounting::{self, Counters, Input, Measured},
    args::{Args, Mode},
    resources,
    view::{Phases, View},
    work,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::{BROWSER_CSS, Document, host::capture::CaptureSession, metrics::FontContext};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

pub(crate) async fn sample(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    step: Step<'_>,
    capture: &CaptureSession,
    luid: &super::work::Adapter,
) -> Result<(Measured, kamin_html::host::capture::Frame, work::Sample)> {
    let (measured, frame, work) =
        sample_with(cx, window, fonts, args, step, capture, luid, true).await?;
    Ok((
        measured,
        frame.ok_or("armed first frame was not captured")?,
        work,
    ))
}

/// `sample`; without `present` the frame may skip its presentation (an
/// edit that changed nothing on screen), and then no frame is captured.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn sample_with(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    step: Step<'_>,
    capture: &CaptureSession,
    luid: &super::work::Adapter,
    present: bool,
) -> Result<(
    Measured,
    Option<kamin_html::host::capture::Frame>,
    work::Sample,
)> {
    let skipped = gpui::layer_counters::PRESENTS_SKIPPED.load(std::sync::atomic::Ordering::Relaxed);
    let (started, before, seq, resources_time, build_time, timings, phases, work) = window
        .update(cx, |view, window, cx| -> Result<_> {
            // Markup of the same shape becomes mutation records before the sample; a change of shape
            // (insertion, removal) enters as one `Document::update` with the new markup, measured.
            let edit = match step {
                Step::Edit(html) => Some((html, view.doc.mutations(html))),
                _ => None,
            };
            // Open counters before the frame: observer_setup is diagnostic cost,
            // outside first_frame_total.
            let setup = Instant::now();
            let work = work::Capture::begin(luid)?;
            let observer_setup = setup.elapsed();
            let started = Instant::now();
            let before = Counters::read(capture);
            let seq = capture.sequence();
            let mut resources_time = None;
            let mut build_time = None;
            let mut timings = None;
            if let Some((html, records)) = &edit {
                let t = kamin_html::work::Clock::now();
                kamin_html::work::phase!(_document, DocumentOther);
                match records {
                    Some(records) => {
                        view.doc.apply(records, BROWSER_CSS)?;
                    }
                    None => {
                        view.doc.update(html, BROWSER_CSS)?;
                    }
                }
                build_time = Some(t.elapsed());
                timings = Some(view.doc.build_timings());
            }
            if let Step::Cold(markup) = step {
                let t = kamin_html::work::Clock::now();
                let mut source = Vec::new();
                let html = {
                    kamin_html::work::phase!(_phase, HostResources);
                    match markup {
                        Some(html) => html.into(),
                        None => resources::read_into(&args.path, &mut source)?,
                    }
                };
                resources_time = Some(t.elapsed());
                let t = kamin_html::work::Clock::now();
                kamin_html::work::phase!(_document, DocumentOther);
                let mut builder = Document::builder(&html, BROWSER_CSS);
                builder
                    .path(&args.path)
                    .fonts(fonts)
                    .viewport((args.viewport.0 as f32, args.viewport.1 as f32));
                // Scroll measures the standalone browser view, whose root scroller
                // takes the wheel (`host::DocumentView`).
                let doc = if args.mode == Mode::Scroll {
                    builder.build_root_scroller()?
                } else {
                    builder.build()?
                };
                build_time = Some(t.elapsed());
                if let Some(limit) = doc.traversal_limit() {
                    return Err(format!("degraded document: {limit}").into());
                }
                timings = Some(doc.build_timings());
                view.doc = doc;
                view.doc.set_animation_elapsed(view.animation_elapsed);
                if let Some(at) = view.fresh_scroll.take() {
                    view.doc.set_root_scroll_offset(at)
                }
            }
            view.phases = Rc::new(RefCell::new(Phases::requested(observer_setup)));
            capture.arm_first();
            cx.notify();
            // The capture reads the presented frame: present it even when a
            // write into the scene layer changed nothing on screen.
            if present {
                window.require_presentation();
            }
            window.refresh();
            Ok((
                started,
                before,
                seq,
                resources_time,
                build_time,
                timings,
                view.phases.clone(),
                work,
            ))
        })??;
    let completed = if present {
        Ok(wait(cx, seq, capture).await?)
    } else {
        wait_or_skip(cx, seq, capture, skipped).await?
    };
    let resumed = Instant::now();
    let total = match completed {
        Ok(_) => resumed - started,
        Err(at) => at.duration_since(started),
    };
    let work = window.read_with(cx, |view, _| {
        // Document phases ran in cold and in edit (`Document::update`) samples.
        work.finish(!matches!(step, Step::Warm), &view.doc)
    })?;
    let after = Counters::read(capture);
    let Ok(completed) = completed else {
        // A skipped presentation: no frame to read, the draw was the frame.
        if phases.borrow().renders != 1 {
            return Err("a skipped presentation without exactly one render".into());
        }
        let out = accounting::partition(
            Input {
                resources: resources_time,
                document: build_time,
                timings,
                layout: Counters::delta(after.layout, before.layout),
                draw: Counters::delta(after.draw, before.draw),
                present: Duration::ZERO,
                readback: Duration::ZERO,
                copy: Duration::ZERO,
                total,
                readback_to_resume: Duration::ZERO,
            },
            &phases.borrow(),
        )?;
        return Ok((out, None, work));
    };
    let frame = capture
        .take_first()
        .ok_or("armed first frame was not captured")?;
    let ambiguous = completed != seq + 1
        || frame.seq != seq + 1
        || after.frames.wrapping_sub(before.frames) != 1
        || phases.borrow().renders != 1;
    // Scroll mode's load is no sample: a page may draw again as it loads.
    if ambiguous && !(args.mode == Mode::Scroll && matches!(step, Step::Cold(_))) {
        return Err(format!(
            "unexpected extra/reused frame: start={seq}, completed={completed}, first={}, latest={}, draws={}, renders={}; refusing an ambiguous phase sample",
            frame.seq,
            capture.sequence(),
            after.frames.wrapping_sub(before.frames),
            phases.borrow().renders,
        ).into());
    }
    let pixels = &frame.pixels;
    if (pixels.width, pixels.height) != args.viewport {
        return Err(format!(
            "capture size {}x{} differs from requested viewport",
            pixels.width, pixels.height
        )
        .into());
    }
    let degradation = window.read_with(cx, |view, _| view.doc.traversal_limit())?;
    if let Some(limit) = degradation {
        return Err(format!("degraded frame: {limit}").into());
    }
    let out = accounting::partition(
        Input {
            resources: resources_time,
            document: build_time,
            timings,
            layout: Counters::delta(after.layout, before.layout),
            draw: Counters::delta(after.draw, before.draw),
            present: Counters::delta(after.present, before.present),
            readback: after
                .readback
                .checked_sub(before.readback)
                .ok_or("readback counter regressed")?,
            copy: after
                .copy
                .checked_sub(before.copy)
                .ok_or("capture copy counter regressed")?,
            total,
            // Browser frames end at the post-render message. Our readback task
            // resumes later; report that wake-up delay separately.
            readback_to_resume: resumed.saturating_duration_since(frame.completed_at),
        },
        &phases.borrow(),
    )?;
    Ok((out, Some(frame), work))
}
