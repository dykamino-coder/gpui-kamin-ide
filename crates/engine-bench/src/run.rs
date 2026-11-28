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
use std::time::Instant;
use waits::wait;
mod measure;
pub(super) use measure::measure;
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
        "device_scale_factor": kamin_html::host::instrumentation::from_env().scale_factor.unwrap_or(1.), "surface": args.surface.name(), "profile": "release", "wpt_harness": kamin_html::BuildTimings::WPT_HARNESS_ENABLED,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "gpu": gpu,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "runs_per_mode": args.runs,
        "animation_step_ms": args.animation_step.map(|t| t.as_secs_f64() * 1000.0),
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
