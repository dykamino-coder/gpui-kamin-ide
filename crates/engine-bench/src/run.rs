//! Один замер на один завершённый кадр; ошибки и лишние кадры не скрываются.

use super::{
    accounting::{self, Counters, Input, Measured},
    args::{Args, Mode},
    frames::Frames,
    host, pixels, report, resources,
    view::{Phases, View},
    work,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::host::capture::CaptureSession;
use kamin_html::{BROWSER_CSS, Document, metrics::FontContext};
use serde_json::json;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Что меняется перед кадром замера.
#[derive(Clone, Copy)]
pub(super) enum Step<'a> {
    /// Новый документ: из файла входа или из данной разметки.
    Cold(Option<&'a str>),
    /// Тот же документ, принудительная перерисовка.
    Warm,
    /// Локальная правка: записи мутаций отличия данной разметки от документа
    /// (`Document::mutations` до начала замера, как DOM-вызов пробы JS у
    /// браузеров), применённые `Document::apply`.
    Edit(&'a str),
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
    step: Step<'_>,
    capture: &CaptureSession,
    luid: (i32, u32),
) -> Result<(Measured, kamin_html::host::capture::Frame, work::Sample)> {
    let (started, before, seq, resources_time, build_time, timings, phases, work) = window
        .update(cx, |view, window, cx| -> Result<_> {
            // Разметка той же формы сводится к записям мутаций до начала замера.
            // Правка формы (вставка, удаление узла) входит в движок одним путём —
            // `Document::update` с новой разметкой; он и меряется.
            let edit = match step {
                Step::Edit(html) => Some((html, view.doc.mutations(html))),
                _ => None,
            };
            let started = Instant::now();
            let work = work::Capture::begin(luid)?;
            let observer_setup = started.elapsed();
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
            view.phases = Rc::new(RefCell::new(Phases::requested(observer_setup)));
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
    let work = window.read_with(cx, |view, _| {
        // Document phases ran in cold and in edit (`Document::update`) samples.
        work.finish(!matches!(step, Step::Warm), &view.doc)
    })?;
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
    startup_timings: &mut super::startup::Startup,
) -> Result<()> {
    startup_timings.mark("async_entry");
    // Пустой стартовый кадр и инициализация GPU не входят в document-cold.
    let seq = capture.sequence();
    window.update(cx, |_, window, _| window.refresh())?;
    wait(cx, seq, capture).await?;
    startup_timings.mark("initial_empty_gpu_frame");
    // Замер начинается в покое платформы (без презентов буста после WM_SIZE).
    startup_timings.settle(cx).await;
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
    // Work samples of each mode stay numbers until the report is printed.
    let mut work_samples = Vec::with_capacity(3);
    let mut frames = Frames::new(args)?;
    let mut snapshot_pending = args.snapshot.as_deref();
    if matches!(args.mode, Mode::Warm | Mode::Relayout | Mode::Edit) {
        sample(cx, window, fonts, args, Step::Cold(None), capture, luid).await?;
    }
    if args.mode == Mode::Edit {
        let (edits, work) = super::edits::run(cx, window, fonts, args, capture, luid).await?;
        modes.insert("edit".into(), edits);
        work_samples.push(("edit", work));
    }
    for &(name, cold) in args.mode.phases() {
        let mut samples = Vec::with_capacity(args.runs);
        let mut diagnostics = Vec::with_capacity(args.runs);
        let mut work = Vec::with_capacity(args.runs);
        let step = super::memory::phase_step(cx, capture, cold).await?;
        for index in 1..=args.runs {
            frames.prepare(cx, window, index)?;
            let (sample, frame, counts) =
                sample(cx, window, fonts, args, step, capture, luid).await?;
            work.push(counts);
            if let Some(path) = snapshot_pending.take() {
                pixels::save(&frame.pixels, path)?;
            }
            frames.observe(cx, window, frame, name, index)?;
            samples.push(sample.phases);
            diagnostics.push(sample.diagnostics);
        }
        #[cfg_attr(not(feature = "heap-report"), allow(unused_mut))]
        let mut mode = report::measured_mode(&samples, &diagnostics);
        #[cfg(feature = "heap-report")]
        super::holders::attach(cx, window, &mut mode)?;
        modes.insert(name.into(), mode);
        work_samples.push((name, work));
    }
    for (name, samples) in &work_samples {
        if let Some(mode) = modes.get_mut(*name) {
            report::attach_work(mode, work::json(samples));
        }
    }
    let mut report = json!({"schema_version": 3, "work_counters_enabled": cfg!(feature = "work-counters"), "input": args.path, "viewport_css_px": args.viewport,
        "device_scale_factor": 1, "surface": args.surface.name(), "profile": "release", "wpt_harness": kamin_html::BuildTimings::WPT_HARNESS_ENABLED,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "gpu": gpu,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "runs_per_mode": args.runs,
        "modes": modes,
        "peak_working_set_bytes": host::peak_memory()?, "startup_peak_working_set_bytes": baseline_memory,
        "startup_process_memory": startup.json(), "startup_timings": startup_timings.report(),
        "scope": "document cold; process, OS file cache, font system and GPU caches remain warm",
        "intervals_overlap": false, "diagnostic_intervals_overlap": true});
    if let Some(fields) = frames.report().as_object() {
        for (key, value) in fields {
            report[key] = value.clone();
        }
    }
    let run = (cx, window, fonts, args, capture, luid);
    frames.finish(&mut report, run).await?;
    // После замера установившейся памяти: запись сборки — куча стенда, не страницы.
    report["build"] = super::build_info::json();
    report::print(&report, args.json)?;
    Ok(())
}
