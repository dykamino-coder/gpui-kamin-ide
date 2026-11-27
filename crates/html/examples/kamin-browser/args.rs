//! Explicit host inputs for reproducing a page's initial viewport and journal.

use std::{ffi::OsString, path::PathBuf};

pub(super) struct Args {
    pub path: PathBuf,
    pub why: Option<PathBuf>,
    pub viewport: (u32, u32),
}

impl Args {
    pub fn parse(values: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut values = values.into_iter();
        let path = values.next().ok_or("expected a page path")?.into();
        let mut args = Self {
            path,
            why: None,
            viewport: (1280, 900),
        };
        while let Some(flag) = values.next() {
            let value = values.next().ok_or("expected an option value")?;
            match flag.to_str() {
                Some("--why") => args.why = Some(value.into()),
                Some("--viewport") => {
                    args.viewport = viewport(value.to_str().ok_or("invalid viewport")?)?;
                }
                _ => return Err("expected --why DIR or --viewport WIDTHxHEIGHT".into()),
            }
        }
        Ok(args)
    }
}

fn viewport(value: &str) -> Result<(u32, u32), String> {
    let (width, height) = value.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let parse = |value: &str| -> Result<u32, String> {
        value
            .parse::<u32>()
            .ok()
            .filter(|axis| (1..=8192).contains(axis))
            .ok_or_else(|| "viewport axes must be integers in 1..8192".into())
    };
    Ok((parse(width)?, parse(height)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_options_preserve_css_constraints_and_paths() {
        let args = Args::parse(
            [
                "a page.html",
                "--viewport",
                "1446x804",
                "--why",
                "a journal",
            ]
            .map(OsString::from),
        )
        .unwrap();
        assert_eq!(args.path, PathBuf::from("a page.html"));
        assert_eq!(args.why, Some(PathBuf::from("a journal")));
        assert_eq!(args.viewport, (1446, 804));
    }

    #[test]
    fn invalid_options_fail_before_a_window_is_opened() {
        for values in [
            vec![],
            vec!["page", "--why"],
            vec!["page", "--other", "value"],
            vec!["page", "--viewport", "0x804"],
            vec!["page", "--viewport", "1446xNaN"],
            vec!["page", "--viewport", "1446x8193"],
        ] {
            assert!(Args::parse(values.into_iter().map(OsString::from)).is_err());
        }
    }
}
