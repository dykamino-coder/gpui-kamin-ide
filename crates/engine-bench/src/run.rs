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

/// Wait for the frame after `sequence`: its readback, or its decision to
/// skip presentation (`gpui::layer_counters::PRESENTS_SKIPPED`), then
/// `None` and the moment it decided.
async fn wait_or_skip(
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

/// Room for one more whole wheel step down on the root scroller.
fn scroll_room(cx: &AsyncApp, window: WindowHandle<View>) -> Result<bool> {
    Ok(window.read_with(cx, |view, _| {
        match (view.doc.root_scroll_offset(), view.doc.root_scroll_max()) {
            (Some(at), Some(max)) => max.1 - at.1 >= super::frames::WHEEL_STEP - 0.5,
            _ => false,
        }
    })?)
}

pub(super) async fn sample(
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
pub(super) async fn sample_with(
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
    let (started, cycles0, before, seq, resources_time, build_time, timings, phases, work) = window
        .update(cx, |view, window, cx| -> Result<_> {
            // Markup of the same shape becomes mutation records before the sample; a change of shape
            // (insertion, removal) enters as one `Document::update` with the new markup, measured.
            let edit = match step {
                Step::Edit(html) => Some((html, view.doc.mutations(html))),
                _ => None,
            };
            // Захват счётчиков открывается до начала кадра: его цена — диагностика
            // observer_setup, а не часть first_frame_total.
            let setup = Instant::now();
            let work = work::Capture::begin(luid)?;
            let observer_setup = setup.elapsed();
            let started = Instant::now();
            let cycles0 = super::process::cycles();
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
                cycles0,
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
    // CPU of every thread of the process inside the frame, for the timing build too:
    // cycle time is exact where process times move in timer quanta (~15.6 ms).
    let cycles = super::process::cycles().saturating_sub(cycles0) as f64 / 1e6;
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
        let mut out = accounting::partition(
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
        out.diagnostics.insert("process_cpu_mcycles", Some(cycles));
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
    let mut out = accounting::partition(
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
            // Конец кадра у браузеров — сообщение после обновления отрисовки; наш
            // отсчёт ждёт ещё пробуждения задачи после readback: оно выносится отдельно.
            readback_to_resume: resumed.saturating_duration_since(frame.completed_at),
        },
        &phases.borrow(),
    )?;
    out.diagnostics.insert("process_cpu_mcycles", Some(cycles));
    Ok((out, Some(frame), work))
}

/// Process state shared by every measured mode: GPU identity and start-up memory.
pub(super) struct Prepared {
    pub(super) gpu: serde_json::Value,
    pub(super) adapter: work::Adapter,
    baseline_memory: u64,
    startup: super::process::Snapshot,
}

/// Initial empty GPU frame, platform quiet and GPU identity: once per process.
pub(super) async fn prepare(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    args: &Args,
    platform: &gpui_windows::WindowsPlatform,
    capture: &CaptureSession,
    startup_timings: &mut super::startup::Startup,
) -> Result<Prepared> {
    startup_timings.mark("async_entry");
    // Пустой стартовый кадр и инициализация GPU не входят в document-cold.
    let seq = capture.sequence();
    window.update(cx, |_, window, _| {
        window.require_presentation();
        window.refresh()
    })?;
    wait(cx, seq, capture).await?;
    startup_timings.mark("initial_empty_gpu_frame");
    // Замер начинается в покое платформы (без презентов буста после WM_SIZE).
    startup_timings.settle(cx).await;
    let mut gpu = serde_json::to_value(window.update(cx, |_, window, _| window.gpu_specs())?)?;
    let identity = platform.gpu_adapter_info()?;
    let adapter = work::Adapter::open((identity.luid_high_part, identity.luid_low_part));
    gpu["requested"] = json!(args.gpu);
    gpu["device_name"] = json!(identity.device_name);
    gpu["driver_version"] = json!(identity.driver_version);
    gpu["luid"] = json!({"high_part": identity.luid_high_part, "low_part": identity.luid_low_part,
        "hex": format!("{:08X}:{:08X}", identity.luid_high_part as u32, identity.luid_low_part)});
    gpu["vendor_id"] = json!(identity.vendor_id);
    gpu["device_id"] = json!(identity.device_id);
    let baseline_memory = host::peak_memory()?;
    let startup = super::process::Snapshot::read()?;
    Ok(Prepared {
        gpu,
        adapter,
        baseline_memory,
        startup,
    })
}

/// The modes of `args` on its input: cold load of the input, then each phase.
pub(super) async fn measure(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    capture: &CaptureSession,
    luid: &work::Adapter,
) -> Result<(serde_json::Map<String, serde_json::Value>, Frames)> {
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
        let (edits, work) = super::edits::run(cx, window, fonts, args, capture, luid).await?;
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
                Some(super::memory::before_phase(cx, capture, args).await?),
            )
        } else {
            super::memory::phase_step(cx, capture, args, cold).await?
        };
        let series = super::series::Series::new(args);
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
            frames.prepare(cx, window, index)?;
            let (sample, frame, counts) =
                sample(cx, window, fonts, args, step, capture, luid).await?;
            let kept = super::frames::keeps_step(args.mode == Mode::Scroll, index);
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
                totals.extend(super::series::total(&sample.phases));
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
        super::holders::attach(cx, window, name, &mut mode)?;
        if let Some(quiet) = &quiet {
            mode["start_busy_steady_windows"] = quiet["busy_windows"].clone();
            mode["start_quiet"] = super::memory::quiet_summary(quiet);
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

/// Report fields of the process, around `modes` (or a plan's phases).
pub(super) fn header(
    args: &Args,
    prepared: &Prepared,
    modes: serde_json::Value,
    startup_timings: &super::startup::Startup,
) -> Result<serde_json::Value> {
    let Prepared {
        gpu,
        baseline_memory,
        startup,
        ..
    } = prepared;
    Ok(
        json!({"schema_version": 3, "work_counters_enabled": cfg!(feature = "work-counters"), "input": args.path, "viewport_css_px": args.viewport,
        "device_scale_factor": 1, "surface": args.surface.name(), "profile": "release", "wpt_harness": kamin_html::BuildTimings::WPT_HARNESS_ENABLED,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "gpu": gpu,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "runs_per_mode": args.runs,
        "modes": modes,
        "peak_working_set_bytes": host::peak_memory()?, "startup_peak_working_set_bytes": baseline_memory,
        "startup_process_memory": startup.json(), "startup_timings": startup_timings.report(),
        "scope": "document cold; process, OS file cache, font system and GPU caches remain warm",
        "intervals_overlap": false, "diagnostic_intervals_overlap": true}),
    )
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
    let prepared = prepare(cx, window, args, platform, capture, startup_timings).await?;
    let luid = &prepared.adapter;
    let (modes, frames) = measure(cx, window, fonts, args, capture, luid).await?;
    let mut report = header(args, &prepared, modes.into(), startup_timings)?;
    if let Some(fields) = frames.report().as_object() {
        for (key, value) in fields {
            report[key] = value.clone();
        }
    }
    let run = (cx, window, fonts, args, capture, luid);
    // A failed check (a frame unlike the fresh document) still fails the run,
    // but after the report: the timings and the per-step checks are kept.
    let finished = frames.finish(&mut report, run).await;
    // После замера установившейся памяти: запись сборки — куча стенда, не страницы.
    report["build"] = super::build_info::json();
    report["cpu_calibration"] = super::process::calibration();
    report["unavailable_resources"] = super::stylesheet::unavailable().into();
    report::print(&report, args.json)?;
    finished
}
