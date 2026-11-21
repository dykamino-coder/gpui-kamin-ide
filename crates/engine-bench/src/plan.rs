//! План (`--plan PLAN.json`): все фазы страницы в одном процессе.
//!
//! Каждая фаза — прежний режим на своём входе (своя холодная загрузка, затем
//! серия кадров), но без окна устойчивой памяти перед фазой: тихое окно
//! только проверяет, что непрошеных кадров нет. Память страницы меряется один
//! раз — после первой фазы (`memory_settle_ms`), как у браузеров после
//! загрузки. Сбой фазы записывается в её отчёт и не отменяет остальные фазы.
//!
//! ```json
//! {"series_ms": 500, "min_runs": 3, "memory_settle_ms": 2000,
//!  "phases": [{"name": "R", "mode": "relayout", "input": "base.html", "runs": 30},
//!             {"name": "C", "mode": "edit", "edit": "class", "input": "C.html"}]}
//! ```
//! `runs` — наибольшая длина серии; относительные входы — от файла плана.

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
        "edit" => Mode::Edit,
        _ => return Err(format!("plan phase mode must be cold, warm, relayout, scroll or edit, not {name:?}").into()),
    })
}

impl Plan {
    pub(super) fn parse(base: &Args, text: &str, dir: &Path) -> Result<Self> {
        let raw: Value = serde_json::from_str(text)?;
        let series_ms = raw["series_ms"].as_u64().unwrap_or(500);
        let min_runs = raw["min_runs"].as_u64().unwrap_or(3) as usize;
        let memory_settle_ms = raw["memory_settle_ms"].as_u64().unwrap_or(0);
        if memory_settle_ms != 0 && !(MEMORY_STEADY_WINDOW_MS..=60_000).contains(&memory_settle_ms) {
            return Err(format!("plan memory_settle_ms must be 0 or {MEMORY_STEADY_WINDOW_MS}..60000").into());
        }
        let mut phases = Vec::new();
        for p in raw["phases"].as_array().ok_or("plan needs a phases array")? {
            let name = p["name"].as_str().ok_or("plan phase needs a name")?.to_owned();
            if phases.iter().any(|x: &Phase| x.name == name) {
                return Err(format!("plan phase {name:?} is given twice").into());
            }
            let mut args = base.clone();
            args.mode = mode(p["mode"].as_str().ok_or("plan phase needs a mode")?)?;
            if let Some(edit) = p["edit"].as_str() {
                args.edit = Edit::parse(edit)?;
            }
            let input = dir.join(p["input"].as_str().ok_or("plan phase needs an input")?);
            args.path = input.canonicalize().map_err(|e| format!("plan input {}: {e}", input.display()))?;
            args.runs = p["runs"].as_u64().unwrap_or(30) as usize;
            if !(1..=100_000).contains(&args.runs) {
                return Err("plan runs must be 1..100000".into());
            }
            args.steady_each_phase = false;
            args.series_ms = series_ms;
            args.min_runs = min_runs;
            args.memory_settle_ms = 0;
            args.plan = None;
            args.snapshot = None;
            phases.push(Phase { name, args });
        }
        if phases.is_empty() {
            return Err("plan has no phases".into());
        }
        Ok(Self { phases, memory_settle_ms, raw })
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
    if let Err(error) = frames.finish(&mut report, (&mut *cx, window, fonts, args, capture, luid)).await {
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
    super::report::print(&report, base.json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(dir: &Path) -> Args {
        let page = dir.join("p.html");
        std::fs::write(&page, "<p>x</p>").expect("write page");
        Args::parse([page.to_string_lossy().into_owned()]).expect("parse").expect("args")
    }

    #[test]
    fn plan_phases_take_their_own_input_and_drop_the_steady_window() {
        let dir = std::env::temp_dir().join(format!("engine-bench-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let b = base(&dir);
        let plan = Plan::parse(&b, r#"{"series_ms": 400, "min_runs": 4, "memory_settle_ms": 2000,
            "phases": [{"name": "R", "mode": "relayout", "input": "p.html", "runs": 12},
                       {"name": "C", "mode": "edit", "edit": "class", "input": "p.html"}]}"#, &dir)
        .expect("plan");
        assert_eq!(plan.phases.len(), 2);
        let r = &plan.phases[0].args;
        assert_eq!((r.mode, r.runs, r.series_ms, r.min_runs, r.steady_each_phase), (Mode::Relayout, 12, 400, 4, false));
        assert_eq!(plan.phases[1].args.edit, Edit::Class);
        assert_eq!(plan.memory_settle_ms, 2000);
        assert!(Plan::parse(&b, r#"{"phases": []}"#, &dir).is_err());
        assert!(Plan::parse(&b, r#"{"phases": [{"name": "X", "mode": "both", "input": "p.html"}]}"#, &dir).is_err());
        assert!(Plan::parse(&b, r#"{"memory_settle_ms": 10, "phases": [{"name": "R", "mode": "warm", "input": "p.html"}]}"#, &dir).is_err());
        assert!(Plan::parse(&b, r#"{"phases": [{"name": "R", "mode": "warm", "input": "missing.html"}]}"#, &dir).is_err());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
