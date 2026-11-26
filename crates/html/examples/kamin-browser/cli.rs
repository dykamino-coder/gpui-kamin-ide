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
        not(feature = "viewer-timing"),
        allow(dead_code, reason = "used by the opt-in timing driver")
    )]
    pub folder: PathBuf,
    #[cfg_attr(
        not(feature = "viewer-timing"),
        allow(dead_code, reason = "used by the opt-in timing driver")
    )]
    pub base_only: bool,
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
        base_only: false,
        width: 800,
        height: 600,
    };
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--base-only") => time.base_only = true,
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
            "--base-only",
            "--height",
            "480",
            "--width",
            "375",
        ])
        .unwrap()
        .time
        .unwrap();
        assert!(time.base_only);
        assert_eq!((time.width, time.height), (375, 480));
        assert!(args(&["--time"]).is_err());
        assert!(args(&["--time", "pages", "--width", "0"]).is_err());
        assert!(args(&["--time", "pages", "--unknown"]).is_err());
    }
}
