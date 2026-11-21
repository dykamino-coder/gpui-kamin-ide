//! Run orchestration; effect time is prepared before measured frames.
mod samples;
mod waits;
use super::{
    args::{Args, Mode},
    frames::Frames,
    host, pixels, report,
    view::View,
    work,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::{host::capture::CaptureSession, metrics::FontContext};
pub(super) use samples::{sample, sample_with};
use serde_json::json;
use waits::wait;
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
    let luid = &adapter;
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
        let (step, busy_windows) = if args.mode == Mode::Scroll {
            let busy = super::memory::converge(cx, capture, 5).await?;
            (Step::Warm, Some(busy))
        } else {
            (super::memory::phase_step(cx, capture, cold).await?, None)
        };
        for index in 1..=args.runs {
            super::animation::prepare(cx, window, args, index)?;
            frames.prepare(cx, window, index)?;
            let (sample, frame, counts) =
                sample(cx, window, fonts, args, step, capture, luid).await?;
            work.push(counts);
            if let Some(path) = snapshot_pending.take() {
                pixels::save(&frame.pixels, path)?;
            }
            frames.observe(cx, window, frame, name, index)?;
            frames
                .settle((&mut *cx, window, fonts, args, capture, luid), step)
                .await?;
            samples.push(sample.phases);
            diagnostics.push(sample.diagnostics);
        }
        #[cfg_attr(not(feature = "heap-report"), allow(unused_mut))]
        let mut mode = report::measured_mode(&samples, &diagnostics);
        #[cfg(feature = "heap-report")]
        super::holders::attach(cx, window, name, &mut mode)?;
        if let Some(busy) = busy_windows {
            mode["start_busy_steady_windows"] = json!(busy);
        }
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
        "animation_step_ms": args.animation_step.map(|t| t.as_secs_f64() * 1000.0),
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
    // A failed check (a frame unlike the fresh document) still fails the run,
    // but after the report: the timings and the per-step checks are kept.
    let finished = frames.finish(&mut report, run).await;
    // После замера установившейся памяти: запись сборки — куча стенда, не страницы.
    report["build"] = super::build_info::json();
    report::print(&report, args.json)?;
    finished
}
