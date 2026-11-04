//! Режимы стенда; сырая история материализуется после последнего замера.

mod deadline;
mod sample;
use super::{
    args::{Args, Mode},
    frames::Frames,
    history::History,
    host, pixels, report,
    view::View,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::{host::capture::CaptureSession, metrics::FontContext};
use sample::wait;
pub(super) use sample::{Step, sample};
use serde_json::json;
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
    wait(seq, capture).await?;
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
    let mut histories = BTreeMap::new();
    let mut frames = Frames::new(args)?;
    let mut snapshot_pending = args.snapshot.as_deref();
    if matches!(args.mode, Mode::Warm | Mode::Relayout | Mode::Edit) {
        sample(cx, window, fonts, args, Step::Cold(None), capture, luid).await?;
    }
    if args.mode == Mode::Edit {
        let edits = super::edits::run(cx, window, fonts, args, capture, luid).await?;
        histories.insert("edit", edits);
    }
    for &(name, cold) in args.mode.phases() {
        let mut history = History::new()?;
        let step = if cold { Step::Cold(None) } else { Step::Warm };
        for index in 1..=args.runs {
            frames.prepare(cx, window, index)?;
            let (sample, frame, (counts, resources)) =
                sample(cx, window, fonts, args, step, capture, luid).await?;
            if let Some(path) = snapshot_pending.take() {
                pixels::save(&frame.pixels, path)?;
            }
            frames.observe(cx, window, frame, name, index)?;
            history.push(sample, counts, resources)?;
        }
        histories.insert(name, history);
    }
    // Память idle/empty-document тоже измеряется до чтения накопленной истории.
    let unchanged = !frames.changed();
    frames.release_baselines();
    let memory = if args.memory_settle_ms > 0 {
        if !unchanged {
            return Err("memory audit requires an unchanged RGB frame sequence".into());
        }
        Some(super::memory::finish(cx, window, fonts, args, capture, luid).await?)
    } else {
        None
    };
    let mut frame_fields = frames.report()?;
    drop(frames);
    let modes: serde_json::Map<_, _> = histories
        .into_iter()
        .map(|(name, history)| Ok((name.to_owned(), history.finish()?)))
        .collect::<std::io::Result<_>>()?;
    let mut report = json!({"schema_version": 3, "work_counters_enabled": cfg!(feature = "work-counters"), "input": args.path, "viewport_css_px": args.viewport,
        "device_scale_factor": 1, "profile": "release", "wpt_harness": kamin_html::BuildTimings::WPT_HARNESS_ENABLED,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "gpu": gpu,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "runs_per_mode": args.runs,
        "peak_working_set_bytes": host::peak_memory()?, "startup_peak_working_set_bytes": baseline_memory,
        "startup_process_memory": startup.json(),
        "scope": "document cold; process, OS file cache, font system and GPU caches remain warm",
        "intervals_overlap": false, "diagnostic_intervals_overlap": true});
    report["modes"] = serde_json::Value::Object(modes);
    if let Some(fields) = frame_fields.as_object_mut() {
        for (key, value) in std::mem::take(fields) {
            report[key] = value;
        }
    }
    if let Some(memory) = memory {
        report["memory_audit"] = memory;
    }
    report::print(&report, args.json)?;
    Ok(())
}
