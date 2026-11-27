//! Command-line parsing keeps double-click startup and local-path arguments intact.

use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Default)]
pub struct Options {
    pub path: Option<PathBuf>,
    pub time: Option<TimeOptions>,
}

#[derive(Debug)]
pub struct TimeOptions {
    #[cfg_attr(
        not(feature = "why-profiler"),
        allow(dead_code, reason = "used by the opt-in timing driver")
    )]
    pub folder: PathBuf,
    #[cfg_attr(
        not(feature = "why-profiler"),
        allow(dead_code, reason = "used by the opt-in journal driver")
    )]
    pub why: PathBuf,
    #[cfg_attr(
        not(feature = "why-profiler"),
        allow(dead_code, reason = "used by the opt-in timing driver")
    )]
    pub base_only: bool,
    pub plain: bool,
    #[cfg_attr(
        not(feature = "why-profiler"),
        allow(dead_code, reason = "opt-in idle observation")
    )]
    pub idle: bool,
    pub width: u32,
    pub height: u32,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Options::default());
    };
    if first != "--time" {
        if args.next().is_some() {
            return Err("Expected one file or folder path.".into());
        }
        return Ok(Options {
            path: Some(first.into()),
            time: None,
        });
    }
    let mut time = TimeOptions {
        folder: args.next().ok_or("Expected a folder after --time.")?.into(),
        why: PathBuf::new(),
        base_only: false,
        plain: false,
        idle: false,
        width: 800,
        height: 600,
    };
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--why") => {
                time.why = args
                    .next()
                    .ok_or("Expected a journal directory after --why.")?
                    .into()
            }
            Some("--base-only") => time.base_only = true,
            Some("--plain") => time.plain = true,
            Some("--idle") => time.idle = true,
            Some("--width" | "--height") => {
                let value = args
                    .next()
                    .and_then(|s| s.to_str().and_then(|s| s.parse::<u32>().ok()))
                    .filter(|value| (1..=16384).contains(value))
                    .ok_or("Viewport dimensions must be integers from 1 to 16384.")?;
                if flag == "--width" {
                    time.width = value;
                } else {
                    time.height = value;
                }
            }
            _ => {
                return Err(format!(
                    "Unknown timing argument: {}",
                    flag.to_string_lossy()
                ));
            }
        }
    }
    if time.why.as_os_str().is_empty() {
        return Err("--time requires --why DIR for the engine journal.".into());
    }
    Ok(Options {
        path: None,
        time: Some(time),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Result<Options, String> {
        parse(values.iter().map(OsString::from))
    }
    #[test]
    fn empty_and_file_startup_remain_available() {
        assert!(args(&[]).unwrap().path.is_none());
        assert_eq!(args(&["a b.html"]).unwrap().path, Some("a b.html".into()));
    }
    #[test]
    fn timing_filter_and_viewport_are_explicit() {
        let time = args(&[
            "--time",
            "pages",
            "--why",
            "journal",
            "--base-only",
            "--plain",
            "--idle",
            "--height",
            "480",
            "--width",
            "375",
        ])
        .unwrap()
        .time
        .unwrap();
        assert_eq!(time.why, PathBuf::from("journal"));
        assert!(time.base_only);
        assert!(time.plain);
        assert!(time.idle);
        assert!(
            !args(&["--time", "pages", "--why", "journal"])
                .unwrap()
                .time
                .unwrap()
                .idle
        );
        assert_eq!((time.width, time.height), (375, 480));
        assert!(args(&["--time"]).is_err());
        assert!(args(&["--time", "pages"]).is_err());
        assert!(args(&["--time", "pages", "--why"]).is_err());
        assert!(args(&["--time", "pages", "--width", "0"]).is_err());
        assert!(args(&["--time", "pages", "--unknown"]).is_err());
    }
}
