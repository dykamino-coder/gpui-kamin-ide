//! Проверяемые параметры CLI; не читает конфигурацию WPT.

use std::path::PathBuf;

/// Наименьшее окно устойчивой памяти (`--memory-settle-ms`) после последнего кадра.
/// mimalloc возвращает освобождённые страницы с задержкой около 1 с, поэтому более
/// короткое окно меряет время кадров, а не память: быстрый кандидат выглядел
/// тяжелее базы. 2 с — то же окно, что у замера браузеров после загрузки.
pub(super) const MEMORY_STEADY_WINDOW_MS: u64 = 2_000;

pub(super) const USAGE: &str = "engine_bench <file.html> [--viewport WxH] [--runs N] \
    [--gpu default|high-performance|minimum-power|software|NAME] [--mode cold|warm|both|relayout|edit] [--edit text|attr|class] [--json] [--snapshot first.png] [--frame-diff-dir DIR] [--fresh-check-dir DIR] [--trace-frames] [--memory-settle-ms N (2000..60000)] [--surface offscreen|window]\n\
    engine_bench --list-gpus\n\
    engine_bench --build-info   (features and profile of this exe, as in the JSON's `build`)\n\
    Default: 800x600, 30 runs per mode, both modes. Windows, release profile only.\n\
    Relayout alternates the CSS viewport width by one pixel on the same Document; capture stays fixed.\n\
    Edit alternates the input and a local edit (--edit, default text) through Document::update,\n\
    then checks each variant pixel-for-pixel against a fresh document.\n\
    Surface: offscreen (default) draws into a GPU texture of a message-only window, never seen\n\
    by the desktop compositor; window parks a presented desktop window off-screen (comparison only).";

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
            Self::Edit => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Args;

    #[test]
    fn rejects_missing_and_empty_gpu_before_opening_the_input() {
        for options in [
            vec!["missing.html", "--gpu"],
            vec!["missing.html", "--gpu", " "],
        ] {
            let error = Args::parse(options.into_iter().map(str::to_owned))
                .err()
                .unwrap();
            assert!(error.to_string().contains("value") || error.to_string().contains("empty"));
        }
    }

    #[test]
    fn memory_audit_requires_five_warm_frames_and_a_bounded_hold() {
        for options in [
            vec!["--runs", "4", "--memory-settle-ms", "2000"],
            vec!["--mode", "cold", "--memory-settle-ms", "2000"],
            vec!["--mode", "relayout", "--memory-settle-ms", "2000"],
            vec!["--memory-settle-ms", "0"],
            vec!["--memory-settle-ms", "60001"],
        ] {
            let error = Args::parse(
                std::iter::once("missing.html")
                    .chain(options)
                    .map(str::to_owned),
            )
            .err()
            .unwrap();
            assert!(error.to_string().contains("memory audit"));
        }
    }

    /// mimalloc отдаёт страницы через ~1 с: окно короче 2 с меряет время кадров.
    #[test]
    fn memory_audit_window_is_at_least_the_two_second_steady_window() {
        let page = std::env::temp_dir().join("engine-bench-settle-window-test.html");
        std::fs::write(&page, "<p>x</p>").unwrap();
        let parse = |ms: &str| {
            Args::parse(
                [page.to_str().unwrap(), "--memory-settle-ms", ms]
                    .into_iter()
                    .map(str::to_owned),
            )
        };
        for short in ["1", "500", "1999"] {
            let error = parse(short).err().unwrap().to_string();
            assert!(error.contains("memory audit hold must be 2000"), "{error}");
        }
        let args = parse("2000").unwrap().unwrap();
        assert_eq!(args.memory_settle_ms, super::MEMORY_STEADY_WINDOW_MS);
        std::fs::remove_file(&page).unwrap();
    }

    #[test]
    fn surface_defaults_to_offscreen_and_rejects_unknown_values() {
        let page = std::env::temp_dir().join("engine-bench-surface-test.html");
        std::fs::write(&page, "<p>x</p>").unwrap();
        let parse = |extra: &[&str]| {
            Args::parse(
                std::iter::once(page.to_str().unwrap())
                    .chain(extra.iter().copied())
                    .map(str::to_owned),
            )
        };
        assert_eq!(
            parse(&[]).unwrap().unwrap().surface,
            super::Surface::Offscreen
        );
        let window = parse(&["--surface", "window"]).unwrap().unwrap();
        assert_eq!(window.surface, super::Surface::Window);
        assert!(parse(&["--surface", "hidden"]).is_err());
        std::fs::remove_file(&page).unwrap();
    }
}

pub(super) struct Args {
    pub(super) path: PathBuf,
    pub(super) gpu: String,
    pub(super) viewport: (u32, u32),
    pub(super) runs: usize,
    pub(super) mode: Mode,
    pub(super) edit: super::edit::Edit,
    pub(super) json: bool,
    pub(super) trace_frames: bool,
    pub(super) memory_settle_ms: u64,
    pub(super) snapshot: Option<PathBuf>,
    pub(super) frame_diff_dir: Option<PathBuf>,
    pub(super) fresh_check_dir: Option<PathBuf>,
    pub(super) surface: Surface,
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
            mode: Mode::Both,
            edit: super::edit::Edit::Text,
            json: false,
            trace_frames: false,
            memory_settle_ms: 0,
            snapshot: None,
            frame_diff_dir: None,
            fresh_check_dir: None,
            surface: Surface::Offscreen,
        };
        while let Some(flag) = args.next() {
            if flag == "--json" {
                out.json = true;
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
                        "edit" => Mode::Edit,
                        _ => return Err("mode must be cold, warm, both, relayout or edit".into()),
                    };
                }
                "--edit" => out.edit = super::edit::Edit::parse(&value)?,
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
                "--frame-diff-dir" => out.frame_diff_dir = Some(value.into()),
                "--fresh-check-dir" => out.fresh_check_dir = Some(value.into()),
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
            && (out.runs < 5 || matches!(out.mode, Mode::Cold | Mode::Relayout | Mode::Edit))
        {
            return Err("memory audit requires at least five unchanged warm frames".into());
        }
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
