//! Один замер на один завершённый кадр; ошибки и лишние кадры не скрываются.

use super::{
    accounting::{self, Input, Measured},
    args::{Args, Mode},
    frames::Frames,
    host, pixels, report, resources,
    view::{Phases, View},
    work,
};
use gpui::{AsyncApp, WindowHandle, frame_perf};
use kamin_html::host::capture::CaptureSession;
use kamin_html::{BROWSER_CSS, Document, metrics::FontContext};
use serde_json::json;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::Ordering::Relaxed,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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

async fn wait(cx: &AsyncApp, sequence: u64, capture: &CaptureSession) -> Result<u64> {
    use futures::future::{Either, select};
    let event = capture.after(sequence);
    let timeout = cx.background_executor().timer(Duration::from_secs(60));
    match select(event, timeout).await {
        Either::Left((Ok(seq), _)) => Ok(seq),
        _ => Err("no completed GPU readback within 60 seconds".into()),
    }
}

pub(super) async fn sample(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    cold: bool,
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
            if cold {
                let t = Instant::now();
                let html = {
                    kamin_html::work::phase!(_phase, HostResources);
                    resources::read(&args.path)?
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
    let completed = wait(cx, seq, capture).await?;
    let total = started.elapsed();
    let work = window.read_with(cx, |view, _| work.finish(cold, &view.doc))?;
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

pub(super) async fn run(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    platform: &gpui_windows::WindowsPlatform,
    capture: &CaptureSession,
) -> Result<()> {
    // Пустой стартовый кадр и инициализация GPU не входят в document-cold.
    let seq = capture.sequence();
    window.update(cx, |_, window, _| window.refresh())?;
    wait(cx, seq, capture).await?;
    let mut gpu = serde_json::to_value(window.update(cx, |_, window, _| window.gpu_specs())?)?;
    let identity = platform.gpu_adapter_info()?;
    let luid = (identity.luid_high_part, identity.luid_low_part);
    gpu["requested"] = json!(args.gpu);
    gpu["device_name"] = json!(identity.device_name);
    gpu["driver_version"] = json!(identity.driver_version);
    gpu["luid"] = json!({"high_part": identity.luid_high_part, "low_part": identity.luid_low_part,
        "hex": format!("{:08X}:{:08X}", identity.luid_high_part as u32, identity.luid_low_part)});
    gpu["vendor_id"] = json!(identity.vendor_id);
    gpu["device_id"] = json!(identity.device_id);
    let baseline_memory = host::peak_memory()?;
    let startup = super::process::Snapshot::read()?;
    let mut modes = serde_json::Map::new();
    let mut frames = Frames::new(args)?;
    let mut snapshot_pending = args.snapshot.as_deref();
    if matches!(args.mode, Mode::Warm | Mode::Relayout) {
        sample(cx, window, fonts, args, true, capture, luid).await?;
    }
    for &(name, cold) in args.mode.phases() {
        let mut samples = Vec::with_capacity(args.runs);
        let mut diagnostics = Vec::with_capacity(args.runs);
        let mut work = Vec::with_capacity(args.runs);
        let mut usage = Vec::with_capacity(args.runs);
        for index in 1..=args.runs {
            frames.prepare(cx, window, index)?;
            let (sample, frame, (counts, resources)) =
                sample(cx, window, fonts, args, cold, capture, luid).await?;
            work.push(counts);
            usage.push(resources);
            if let Some(path) = snapshot_pending.take() {
                pixels::save(&frame.pixels, path)?;
            }
            frames.observe(cx, window, frame, name, index)?;
            samples.push(sample.phases);
            diagnostics.push(sample.diagnostics);
        }
        let mut mode = report::mode(&samples);
        mode["diagnostic_scopes"] = report::mode(&diagnostics);
        mode["work_counters"] = serde_json::json!({"samples": work});
        mode["process_resources"] = serde_json::json!({"samples": usage});
        modes.insert(name.into(), mode);
    }
    let mut report = json!({"schema_version": 3, "work_counters_enabled": cfg!(feature = "work-counters"), "input": args.path, "viewport_css_px": args.viewport,
        "device_scale_factor": 1, "profile": "release", "wpt_harness": kamin_html::BuildTimings::WPT_HARNESS_ENABLED,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "gpu": gpu,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "runs_per_mode": args.runs,
        "modes": modes,
        "peak_working_set_bytes": host::peak_memory()?, "startup_peak_working_set_bytes": baseline_memory,
        "startup_process_memory": startup.json(),
        "scope": "document cold; process, OS file cache, font system and GPU caches remain warm",
        "intervals_overlap": false, "diagnostic_intervals_overlap": true});
    if let Some(fields) = frames.report().as_object() {
        for (key, value) in fields {
            report[key] = value.clone();
        }
    }
    drop(frames);
    if args.memory_settle_ms > 0 {
        if report["frame_pixels_changed"] != false {
            return Err("memory audit requires an unchanged RGB frame sequence".into());
        }
        report["memory_audit"] =
            super::memory::finish(cx, window, fonts, args, capture, luid).await?;
    }
    report::print(&report, args.json)?;
    Ok(())
}
