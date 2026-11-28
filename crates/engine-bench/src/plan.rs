//! Plan (`--plan PLAN.json`): every phase of a page in one process.
//!
//! Each phase is the old mode on its own input (its own cold load, then a
//! frame series), without the steady-memory window before the phase: a quiet
//! window only waits out late frames (`memory.rs`). Page memory is measured once,
//! after the first phase (`memory_settle_ms`), as browsers after their
//! load. A failed phase is written into its report and does not cancel the others.
//!
//! ```json
//! {"series_ms": 500, "min_runs": 3, "memory_settle_ms": 2000,
//!  "phases": [{"name": "R", "mode": "relayout", "input": "base.html", "runs": 30},
//!             {"name": "C", "mode": "edit", "edit": "class", "input": "C.html"}]}
//! ```
//! `runs` is the longest series; a phase may set its own `series_ms`,
//! `min_runs` and `stable_mad`; relative inputs are resolved from the plan file.

use super::{
    args::{Args, MEMORY_STEADY_WINDOW_MS, Mode},
    edit::Edit,
    run,
    view::View,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::{host::capture::CaptureSession, metrics::FontContext};
use serde_json::{Value, json};
use std::path::Path;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) struct Phase {
    name: String,
    args: Args,
}

pub(super) struct Plan {
    phases: Vec<Phase>,
    memory_settle_ms: u64,
    raw: Value,
}

fn mode(name: &str) -> Result<Mode> {
    Ok(match name {
        "cold" => Mode::Cold,
        "warm" => Mode::Warm,
        "relayout" => Mode::Relayout,
        "scroll" => Mode::Scroll,
        "smooth-scroll" => Mode::Smooth,
        "edit" => Mode::Edit,
        _ => {
            return Err(format!(
                "plan phase mode must be cold, warm, relayout, scroll or edit, not {name:?}"
            )
            .into());
        }
    })
}

impl Plan {
    pub(super) fn parse(base: &Args, text: &str, dir: &Path) -> Result<Self> {
        let raw: Value = serde_json::from_str(text)?;
        let series_ms = raw["series_ms"].as_u64().unwrap_or(500);
        let min_runs = raw["min_runs"].as_u64().unwrap_or(3) as usize;
        let stable_mad = raw["stable_mad"].as_f64().unwrap_or(0.0);
        let memory_settle_ms = raw["memory_settle_ms"].as_u64().unwrap_or(0);
        if memory_settle_ms != 0 && !(MEMORY_STEADY_WINDOW_MS..=60_000).contains(&memory_settle_ms)
        {
            return Err(format!(
                "plan memory_settle_ms must be 0 or {MEMORY_STEADY_WINDOW_MS}..60000"
            )
            .into());
        }
        let mut phases = Vec::new();
        for p in raw["phases"]
            .as_array()
            .ok_or("plan needs a phases array")?
        {
            let name = p["name"]
                .as_str()
                .ok_or("plan phase needs a name")?
                .to_owned();
            if phases.iter().any(|x: &Phase| x.name == name) {
                return Err(format!("plan phase {name:?} is given twice").into());
            }
            let mut args = base.clone();
            args.mode = mode(p["mode"].as_str().ok_or("plan phase needs a mode")?)?;
            if let Some(edit) = p["edit"].as_str() {
                args.edit = Edit::parse(edit)?;
            }
            let input = dir.join(p["input"].as_str().ok_or("plan phase needs an input")?);
            args.path = input
                .canonicalize()
                .map_err(|e| format!("plan input {}: {e}", input.display()))?;
            args.runs = p["runs"].as_u64().unwrap_or(30) as usize;
            if !(1..=100_000).contains(&args.runs) {
                return Err("plan runs must be 1..100000".into());
            }
            args.steady_each_phase = false;
            // A phase may set its own series (scroll: as many steps as the page allows, down to one).
            args.series_ms = p["series_ms"].as_u64().unwrap_or(series_ms);
            args.min_runs = p["min_runs"].as_u64().map_or(min_runs, |n| n as usize);
            args.stable_mad = p["stable_mad"].as_f64().unwrap_or(stable_mad);
            if !(0.0..1.0).contains(&args.stable_mad) {
                return Err("plan stable_mad must be 0 (off) or a fraction below 1".into());
            }
            args.memory_settle_ms = 0;
            args.plan = None;
            args.snapshot = None;
            super::args::animation::validate(&args)?;
            phases.push(Phase { name, args });
        }
        if phases.is_empty() {
            return Err("plan has no phases".into());
        }
        Ok(Self {
            phases,
            memory_settle_ms,
            raw,
        })
    }

    fn read(base: &Args) -> Result<Self> {
        let path = base.plan.as_deref().ok_or("no plan file")?;
        let dir = path.parent().unwrap_or(Path::new("."));
        Self::parse(base, &std::fs::read_to_string(path)?, dir)
    }
}

/// One phase: its modes, frame checks and the error of a failed check, kept with the timings.
async fn phase(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    capture: &CaptureSession,
    luid: &super::work::Adapter,
) -> Result<Value> {
    let (modes, frames) = run::measure(cx, window, fonts, args, capture, luid).await?;
    let mut report = json!({"input": args.path, "mode": format!("{:?}", args.mode).to_lowercase(), "modes": modes});
    if let Some(fields) = frames.report().as_object() {
        for (key, value) in fields {
            report[key] = value.clone();
        }
    }
    if let Err(error) = frames
        .finish(&mut report, (&mut *cx, window, fonts, args, capture, luid))
        .await
    {
        report["check_error"] = json!(error.to_string());
    }
    Ok(report)
}

pub(super) async fn run(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    base: &Args,
    platform: &gpui_windows::WindowsPlatform,
    capture: &CaptureSession,
    startup_timings: &mut super::startup::Startup,
) -> Result<()> {
    let plan = Plan::read(base)?;
    let prepared = run::prepare(cx, window, base, platform, capture, startup_timings).await?;
    let luid = &prepared.adapter;
    let mut phases = serde_json::Map::new();
    let mut memory = Value::Null;
    for (index, p) in plan.phases.iter().enumerate() {
        let started = std::time::Instant::now();
        let mut report = match phase(cx, window, fonts, &p.args, capture, luid).await {
            Ok(report) => report,
            Err(error) => json!({"input": p.args.path, "error": error.to_string()}),
        };
        report["wall_ms"] = json!(started.elapsed().as_secs_f64() * 1000.);
        let ok = report.get("error").is_none();
        phases.insert(p.name.clone(), report);
        if index == 0 && ok && plan.memory_settle_ms > 0 {
            memory = match super::memory::page_audit(cx, capture, plan.memory_settle_ms).await {
                Ok(page) => json!({"page": page, "after_phase": p.name,
                    "scope": "live window after the first phase; host idle release only"}),
                Err(error) => json!({"error": error.to_string(), "after_phase": p.name}),
            };
        }
    }
    let mut report = run::header(base, &prepared, Value::Null, startup_timings)?;
    if let Some(object) = report.as_object_mut() {
        object.remove("modes");
    }
    report["phases"] = Value::Object(phases);
    report["memory_audit"] = memory;
    report["plan"] = plan.raw;
    report["build"] = super::build_info::json();
    report["cpu_calibration"] = super::process::calibration();
    report["unavailable_resources"] = super::stylesheet::unavailable().into();
    Ok(super::report::print(&report, base.json)?)
}

#[cfg(test)]
mod tests;
