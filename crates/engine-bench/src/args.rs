//! Проверяемые параметры CLI; не читает конфигурацию WPT.

pub(super) mod animation;
use std::path::PathBuf;

/// Наименьшее окно устойчивой памяти (`--memory-settle-ms`) после последнего кадра.
/// mimalloc возвращает освобождённые страницы с задержкой около 1 с, поэтому более
/// короткое окно меряет время кадров, а не память: быстрый кандидат выглядел
/// тяжелее базы. 2 с — то же окно, что у замера браузеров после загрузки.
pub(super) const MEMORY_STEADY_WINDOW_MS: u64 = 2_000;

mod usage;
pub(super) use usage::USAGE;
/// Куда рисует стенд: текстура message-only окна (DWM его не видит) или
/// прежнее презентуемое окно за краем экрана — только для сверки путей.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Surface {
    Offscreen,
    Window,
}

#[cfg(windows)]
impl Surface {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Offscreen => "offscreen",
            Self::Window => "window",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mode {
    Cold,
    Warm,
    Both,
    Relayout,
    /// One wheel step on the root scroller before each frame (`frames::WHEEL_STEP`).
    Scroll,
    /// Локальная правка разметки между кадрами (`edit::Edit`, `edits`).
    Edit,
}

#[cfg(windows)]
impl Mode {
    pub(super) fn phases(self) -> &'static [(&'static str, bool)] {
        match self {
            Self::Cold => &[("cold", true)],
            Self::Warm => &[("warm", false)],
            Self::Both => &[("cold", true), ("warm", false)],
            Self::Relayout => &[("relayout", false)],
            Self::Scroll => &[("scroll", false)],
            Self::Edit => &[],
        }
    }
}

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(super) struct Args {
    pub(super) path: PathBuf,
    pub(super) gpu: String,
    pub(super) viewport: (u32, u32),
    pub(super) runs: usize,
    pub(super) animation_step: Option<std::time::Duration>,
    pub(super) mode: Mode,
    pub(super) edit: super::edit::Edit,
    pub(super) edit_markup: Option<PathBuf>,
    pub(super) json: bool,
    pub(super) trace_frames: bool,
    /// Scroll: each wheel step of `scroll_step` lands on the root's scrollbar
    /// edge, which no nested scroller covers, and turns at either end of the
    /// document, so a long run crosses it down and up again.
    pub(super) scroll_back: bool,
    pub(super) scroll_step: f32,
    pub(super) memory_settle_ms: u64,
    pub(super) snapshot: Option<PathBuf>,
    pub(super) frame_diff_dir: Option<PathBuf>,
    pub(super) fresh_check_dir: Option<PathBuf>,
    /// DevTools `why` journal of each sampled frame into this directory (`why`).
    pub(super) why: Option<PathBuf>,
    pub(super) surface: Surface,
    /// Every phase of this plan file in one process (`plan.rs`).
    pub(super) plan: Option<PathBuf>,
    /// Memory steady window before each warm phase (the default); a plan replaces it
    /// with a short quiet window that waits out late frames (`memory::settle`).
    pub(super) steady_each_phase: bool,
    /// Frame series: at least `min_runs` samples and `series_ms` of samples, at most
    /// `runs`; 0 = exactly `runs` (the default).
    pub(super) series_ms: u64,
    pub(super) min_runs: usize,
    /// A timed series also stops once its frame totals are stable: relative MAD
    /// (median absolute deviation over the median) at most this; 0 = off.
    pub(super) stable_mad: f64,
}

impl Args {
    pub(super) fn parse(
        args: impl IntoIterator<Item = String>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let mut args = args.into_iter();
        let Some(path) = args.next() else {
            return Ok(None);
        };
        if path == "--help" || path == "-h" {
            return Ok(None);
        }
        let mut out = Self {
            path: path.into(),
            gpu: "default".into(),
            viewport: (800, 600),
            runs: 30,
            animation_step: None,
            mode: Mode::Both,
            edit: super::edit::Edit::Text,
            edit_markup: None,
            json: false,
            trace_frames: false,
            scroll_back: false,
            scroll_step: 120.0,
            memory_settle_ms: 0,
            snapshot: None,
            frame_diff_dir: None,
            fresh_check_dir: None,
            why: None,
            surface: Surface::Offscreen,
            plan: None,
            steady_each_phase: true,
            series_ms: 0,
            min_runs: 1,
            stable_mad: 0.0,
        };
        while let Some(flag) = args.next() {
            if flag == "--json" {
                out.json = true;
                continue;
            }
            if flag == "--scroll-back" {
                out.scroll_back = true;
                continue;
            }
            if flag == "--trace-frames" {
                out.trace_frames = true;
                continue;
            }
            let value = args.next().ok_or("missing option value")?;
            match flag.as_str() {
                "--viewport" => {
                    let (w, h) = value.split_once('x').ok_or("viewport must be WxH")?;
                    out.viewport = (w.parse()?, h.parse()?);
                    if !(1..=8192).contains(&out.viewport.0)
                        || !(1..=8192).contains(&out.viewport.1)
                    {
                        return Err("viewport dimensions must be 1..8192 CSS pixels".into());
                    }
                }
                "--animation-step-ms" => {
                    if out
                        .animation_step
                        .replace(animation::parse(&value)?)
                        .is_some()
                    {
                        return Err("--animation-step-ms is given twice".into());
                    }
                }
                "--runs" => {
                    out.runs = value.parse()?;
                    if !(1..=100_000).contains(&out.runs) {
                        return Err("runs must be 1..100000".into());
                    }
                }
                "--mode" => {
                    out.mode = match value.as_str() {
                        "cold" => Mode::Cold,
                        "warm" => Mode::Warm,
                        "both" => Mode::Both,
                        "relayout" => Mode::Relayout,
                        "scroll" => Mode::Scroll,
                        "edit" => Mode::Edit,
                        _ => {
                            return Err(
                                "mode must be cold, warm, both, relayout, scroll or edit".into()
                            );
                        }
                    };
                }
                "--edit" => out.edit = super::edit::Edit::parse(&value)?,
                "--edit-markup" => {
                    if out.edit_markup.replace(value.into()).is_some() {
                        return Err("--edit-markup is given twice".into());
                    }
                }
                "--gpu" if !value.trim().is_empty() => out.gpu = value,
                "--gpu" => return Err("GPU selector must not be empty".into()),
                "--snapshot" => out.snapshot = Some(value.into()),
                "--memory-settle-ms" => {
                    out.memory_settle_ms = value.parse()?;
                    if !(MEMORY_STEADY_WINDOW_MS..=60_000).contains(&out.memory_settle_ms) {
                        return Err(format!(
                            "memory audit hold must be {MEMORY_STEADY_WINDOW_MS}..60000 milliseconds"
                        )
                        .into());
                    }
                }
                "--plan" => out.plan = Some(value.into()),
                "--scroll-step" => {
                    out.scroll_step = value.parse()?;
                    if !out.scroll_step.is_finite() || out.scroll_step < 0.0 {
                        return Err("scroll step must be finite and nonnegative".into());
                    }
                }
                "--frame-diff-dir" => out.frame_diff_dir = Some(value.into()),
                "--fresh-check-dir" => out.fresh_check_dir = Some(value.into()),
                "--why" => out.why = Some(value.into()),
                "--surface" => {
                    out.surface = match value.as_str() {
                        "offscreen" => Surface::Offscreen,
                        "window" => Surface::Window,
                        _ => return Err("surface must be offscreen or window".into()),
                    };
                }
                _ => return Err(format!("unknown option: {flag}").into()),
            }
        }
        if out.memory_settle_ms > 0
            && (out.runs < 5
                || matches!(
                    out.mode,
                    Mode::Cold | Mode::Relayout | Mode::Scroll | Mode::Edit
                ))
        {
            return Err("memory audit requires at least five unchanged warm frames".into());
        }
        out.animation_step
            .get_or_insert(std::time::Duration::from_micros(16_667));
        animation::validate(&out)?;
        out.path = out.path.canonicalize()?;
        if !out.path.is_file()
            || !out
                .path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
        {
            return Err("input must be a local .html or .htm file".into());
        }
        Ok(Some(out))
    }
}
