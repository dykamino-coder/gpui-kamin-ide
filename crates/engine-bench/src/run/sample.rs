//! Один завершённый кадр: создание документа, счётчики, readback и проверка фаз.

use super::super::{
    accounting::{self, Input, Measured},
    args::Args,
    resources,
    view::{Phases, View},
    work,
};
use super::Result;
use gpui::{AsyncApp, WindowHandle, frame_perf};
use kamin_html::{BROWSER_CSS, Document, host::capture::CaptureSession, metrics::FontContext};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::Ordering::Relaxed,
    time::{Duration, Instant},
};

/// Что меняется перед кадром замера.
#[derive(Clone, Copy)]
pub(crate) enum Step<'a> {
    /// Новый документ: из файла входа или из данной разметки.
    Cold(Option<&'a str>),
    /// Тот же документ, принудительная перерисовка.
    Warm,
    /// `Document::update` данной разметкой (локальная правка).
    Update(&'a str),
}

impl Step<'_> {
    fn cold(self) -> bool {
        matches!(self, Self::Cold(_))
    }
}

struct Counters {
    draw: u32,
    present: u32,
    layout: u32,
    frames: u32,
    readback: Duration,
}

impl Counters {
    fn read(capture: &CaptureSession) -> Self {
        Self {
            draw: frame_perf::DRAW_US.load(Relaxed),
            present: frame_perf::PRESENT_US.load(Relaxed),
            layout: frame_perf::TAFFY_US.load(Relaxed),
            frames: frame_perf::FRAMES.load(Relaxed),
            readback: capture.readback_time(),
        }
    }
    fn delta(current: u32, previous: u32) -> Duration {
        Duration::from_micros(u64::from(current.wrapping_sub(previous)))
    }
}

pub(super) async fn wait(sequence: u64, capture: &CaptureSession) -> Result<u64> {
    match super::deadline::within(capture.after(sequence), Duration::from_secs(60)).await {
        Some(Ok(seq)) => Ok(seq),
        _ => Err("no completed GPU readback within 60 seconds".into()),
    }
}

pub(crate) async fn sample(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    step: Step<'_>,
    capture: &CaptureSession,
    luid: (i32, u32),
) -> Result<(
    Measured,
    kamin_html::host::capture::Frame,
    (serde_json::Value, serde_json::Value),
)> {
    let (started, before, seq, resources_time, build_time, timings, phases, work) = window
        .update(cx, |view, window, cx| -> Result<_> {
            let started = Instant::now();
            let work = work::Capture::begin(luid)?;
            let before = Counters::read(capture);
            let seq = capture.sequence();
            let mut resources_time = None;
            let mut build_time = None;
            let mut timings = None;
            if let Step::Update(html) = step {
                let t = Instant::now();
                kamin_html::work::phase!(_document, DocumentOther);
                view.doc.update(html, BROWSER_CSS)?;
                build_time = Some(t.elapsed());
                timings = Some(view.doc.build_timings());
            }
            if let Step::Cold(markup) = step {
                let t = Instant::now();
                let html = {
                    kamin_html::work::phase!(_phase, HostResources);
                    match markup {
                        Some(html) => html.to_owned(),
                        None => resources::read(&args.path)?,
                    }
                };
                resources_time = Some(t.elapsed());
                let t = Instant::now();
                kamin_html::work::phase!(_document, DocumentOther);
                let doc = Document::builder(&html, BROWSER_CSS)
                    .path(&args.path)
                    .fonts(fonts)
                    .build()?;
                build_time = Some(t.elapsed());
                if let Some(limit) = doc.traversal_limit() {
                    return Err(format!("degraded document: {limit}").into());
                }
                timings = Some(doc.build_timings());
                view.doc = doc;
            }
            view.phases = Rc::new(RefCell::new(Phases::default()));
            capture.arm_first();
            cx.notify();
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
    let completed = wait(seq, capture).await?;
    let total = started.elapsed();
    let work = window.read_with(cx, |view, _| work.finish(step.cold(), &view.doc))?;
    let after = Counters::read(capture);
    let frame = capture
        .take_first()
        .ok_or("armed first frame was not captured")?;
    if completed != seq + 1
        || frame.seq != seq + 1
        || after.frames.wrapping_sub(before.frames) != 1
        || phases.borrow().renders != 1
    {
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
            total,
        },
        &phases.borrow(),
    )?;
    Ok((out, frame, work))
}
