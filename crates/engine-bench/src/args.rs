//! Проверяемые параметры CLI; не читает конфигурацию WPT.

use std::path::PathBuf;

pub(super) const USAGE: &str = "engine_bench <file.html> [--viewport WxH] [--runs N] \
    [--gpu default|high-performance|minimum-power|NAME] [--mode cold|warm|both] [--json] [--snapshot first.png] [--frame-diff-dir DIR]\n\
    engine_bench --list-gpus\n\
    Default: 800x600, 30 runs per mode, both modes. Windows, release profile only.";

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mode {
    Cold,
    Warm,
    Both,
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
}

pub(super) struct Args {
    pub(super) path: PathBuf,
    pub(super) gpu: String,
    pub(super) viewport: (u32, u32),
    pub(super) runs: usize,
    pub(super) mode: Mode,
    pub(super) json: bool,
    pub(super) snapshot: Option<PathBuf>,
    pub(super) frame_diff_dir: Option<PathBuf>,
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
            json: false,
            snapshot: None,
            frame_diff_dir: None,
        };
        while let Some(flag) = args.next() {
            if flag == "--json" {
                out.json = true;
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
                        _ => return Err("mode must be cold, warm or both".into()),
                    };
                }
                "--gpu" if !value.trim().is_empty() => out.gpu = value,
                "--gpu" => return Err("GPU selector must not be empty".into()),
                "--snapshot" => out.snapshot = Some(value.into()),
                "--frame-diff-dir" => out.frame_diff_dir = Some(value.into()),
                _ => return Err(format!("unknown option: {flag}").into()),
            }
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
