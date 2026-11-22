//! Deterministic animation input; elapsed is separate from CPU/GPU timing.

use super::Args;
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) fn parse(value: &str) -> Result<Duration> {
    let milliseconds: f64 = value.parse()?;
    if !milliseconds.is_finite() || milliseconds < 0.0 {
        return Err("animation step must be finite and nonnegative".into());
    }
    Ok(Duration::try_from_secs_f64(milliseconds / 1000.0)?)
}

pub(crate) fn validate(args: &Args) -> Result<()> {
    if let Some(step) = args.animation_step {
        step.checked_mul(u32::try_from(args.runs)?)
            .ok_or("animation series elapsed overflow")?;
    }
    Ok(())
}

impl Args {
    pub(crate) fn animation_time(&self, index: usize) -> Option<Duration> {
        self.animation_step.map(|step| {
            step.checked_mul(u32::try_from(index).expect("validated frame index"))
                .expect("validated animation series elapsed")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_step_accepts_zero_and_fractional_milliseconds() {
        assert_eq!(parse("0").unwrap(), Duration::ZERO);
        assert_eq!(parse("16.25").unwrap(), Duration::from_micros(16_250));
        for value in ["NaN", "inf", "-1", "1e99", "bad"] {
            assert!(parse(value).is_err(), "{value}");
        }
    }

    #[test]
    fn cli_bounds_series_and_preserves_explicit_step() {
        for options in [
            vec!["--animation-step-ms", "1e20", "--runs", "100000"],
            vec!["--animation-step-ms", "0", "--animation-step-ms", "1"],
        ] {
            let error = Args::parse(
                std::iter::once("missing.html")
                    .chain(options)
                    .map(str::to_owned),
            )
            .err()
            .unwrap()
            .to_string();
            assert!(
                error.contains("overflow") || error.contains("requires") || error.contains("twice"),
                "{error}"
            );
        }
        let path = std::env::temp_dir().join(format!(
            "engine-bench-animation-input-{}.html",
            std::process::id()
        ));
        std::fs::write(&path, "<p>animation</p>").unwrap();
        let args = Args::parse(
            [
                path.to_str().unwrap(),
                "--animation-step-ms",
                "16.25",
                "--mode",
                "edit",
                "--fresh-check-dir",
                "references",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap()
        .unwrap();
        assert_eq!(args.animation_time(0), Some(Duration::ZERO));
        assert_eq!(args.animation_time(3), Some(Duration::from_micros(48_750)));
        std::fs::remove_file(path).unwrap();
    }
}
