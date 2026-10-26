//! Один замер на один завершённый кадр; ошибки и лишние кадры не скрываются.

use super::{
    args::{Args, Mode},
    host,
    report::{self, Sample, ms},
    resources,
    view::{Phases, View},
};
use gpui::{AsyncApp, WindowHandle, frame_capture, frame_perf};
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
    fn read() -> Self {
        Self {
            draw: frame_perf::DRAW_US.load(Relaxed),
            present: frame_perf::PRESENT_US.load(Relaxed),
            layout: frame_perf::TAFFY_US.load(Relaxed),
            frames: frame_perf::FRAMES.load(Relaxed),
            readback: frame_capture::readback_time(),
        }
    }
    fn delta_ms(current: u32, previous: u32) -> f64 {
        f64::from(current.wrapping_sub(previous)) / 1000.
    }
}

async fn wait(cx: &AsyncApp, sequence: u64) -> Result<u64> {
    use futures::future::{Either, select};
    let event = frame_capture::after(sequence);
    let timeout = cx.background_executor().timer(Duration::from_secs(60));
    match select(event, timeout).await {
        Either::Left((Ok(seq), _)) => Ok(seq),
        _ => Err("no completed GPU readback within 60 seconds".into()),
    }
}

fn save(frame: &frame_capture::CapturedFrame, path: &std::path::Path) -> Result<()> {
    let mut rgba = frame.bgra.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let mut encoder = png::Encoder::new(std::fs::File::create(path)?, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    Ok(())
}

fn fingerprint(frame: &frame_capture::CapturedFrame) -> String {
    let hash = frame
        .bgra
        .chunks_exact(4)
        .flat_map(|p| &p[..3])
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("{hash:016x}")
}

async fn sample(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    cold: bool,
) -> Result<(Sample, frame_capture::CapturedFrame)> {
    let (started, before, seq, resources_time, build_time, timings, phases) =
        window.update(cx, |view, window, cx| -> Result<_> {
            let started = Instant::now();
            let before = Counters::read();
            let seq = frame_capture::sequence();
            let mut resources_time = None;
            let mut build_time = None;
            let mut timings = None;
            if cold {
                let t = Instant::now();
                let html = resources::read(&args.path)?;
                resources_time = Some(t.elapsed());
                let t = Instant::now();
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
            frame_capture::arm_first();
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
            ))
        })??;
    let completed = wait(cx, seq).await?;
    let total = started.elapsed();
    let after = Counters::read();
    let frame = frame_capture::take_first().ok_or("armed first frame was not captured")?;
    if completed != seq + 1
        || frame.seq != seq + 1
        || after.frames.wrapping_sub(before.frames) != 1
        || phases.borrow().renders != 1
    {
        return Err("unexpected extra/reused frame; refusing an ambiguous phase sample".into());
    }
    if (frame.width, frame.height) != args.viewport {
        return Err(format!(
            "capture size {}x{} differs from requested viewport",
            frame.width, frame.height
        )
        .into());
    }
    let degradation = window.read_with(cx, |view, _| view.doc.traversal_limit())?;
    if let Some(limit) = degradation {
        return Err(format!("degraded frame: {limit}").into());
    }
    let phase = phases.borrow();
    let out = Sample::from([
        ("host_resources", resources_time.map(ms)),
        ("parse", timings.map(|t| ms(t.parse))),
        ("style", timings.map(|t| ms(t.style))),
        ("document_build_total", build_time.map(ms)),
        ("build_elements", Some(ms(phase.build))),
        ("layout_request", Some(ms(phase.request_layout))),
        (
            "layout",
            Some(Counters::delta_ms(after.layout, before.layout)),
        ),
        ("prepaint", Some(ms(phase.prepaint))),
        ("paint", Some(ms(phase.paint))),
        (
            "cpu_draw_total",
            Some(Counters::delta_ms(after.draw, before.draw)),
        ),
        (
            "raster_gpu_readback",
            Some(Counters::delta_ms(after.present, before.present)),
        ),
        (
            "gpu_readback",
            Some(ms(after.readback.saturating_sub(before.readback))),
        ),
        ("first_frame_total", Some(ms(total))),
    ]);
    Ok((out, frame))
}

pub(super) async fn run(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
) -> Result<()> {
    // Пустой стартовый кадр и инициализация GPU не входят в document-cold.
    let seq = frame_capture::sequence();
    window.update(cx, |_, window, _| window.refresh())?;
    wait(cx, seq).await?;
    let gpu = window.update(cx, |_, window, _| window.gpu_specs())?;
    let baseline_memory = host::peak_memory()?;
    let mut modes = serde_json::Map::new();
    let mut first_hash = None;
    let mut snapshot_pending = args.snapshot.as_deref();
    if args.mode == Mode::Warm {
        sample(cx, window, fonts, args, true).await?;
    }
    for (name, cold) in [("cold", true), ("warm", false)] {
        if (cold && args.mode == Mode::Warm) || (!cold && args.mode == Mode::Cold) {
            continue;
        }
        let mut samples = Vec::with_capacity(args.runs);
        for _ in 0..args.runs {
            let (sample, frame) = sample(cx, window, fonts, args, cold).await?;
            let hash = fingerprint(&frame);
            if first_hash.as_ref().is_some_and(|first| *first != hash) {
                return Err(
                    "pixels changed between runs; use a static deterministic document".into(),
                );
            }
            first_hash = Some(hash);
            if let Some(path) = snapshot_pending.take() {
                save(&frame, path)?;
            }
            samples.push(sample);
        }
        modes.insert(name.into(), report::mode(&samples));
    }
    let report = json!({"schema_version": 1, "input": args.path, "viewport_css_px": args.viewport,
        "device_scale_factor": 1, "profile": "release", "wpt_harness": kamin_html::BuildTimings::WPT_HARNESS_ENABLED,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "gpu": gpu,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "runs_per_mode": args.runs, "modes": modes, "first_frame_bgr_fnv1a64": first_hash,
        "peak_working_set_bytes": host::peak_memory()?, "startup_peak_working_set_bytes": baseline_memory,
        "scope": "document cold; process, OS file cache, font system and GPU caches remain warm",
        "intervals_overlap": true});
    report::print(&report, args.json)?;
    Ok(())
}
