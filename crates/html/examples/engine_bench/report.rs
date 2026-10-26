//! Сырые измерения и воспроизводимые статистики, без отбрасывания выбросов.

use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) type Sample = BTreeMap<&'static str, Option<f64>>;

pub(super) fn ms(time: std::time::Duration) -> f64 {
    time.as_secs_f64() * 1000.
}

fn statistics(mut values: Vec<f64>) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let median = if n.is_multiple_of(2) {
        (values[n / 2 - 1] + values[n / 2]) / 2.
    } else {
        values[n / 2]
    };
    let p90 = values[(9 * n).div_ceil(10) - 1];
    json!({ "median_ms": median, "p90_ms": p90, "min_ms": values[0],
        "max_ms": values[n - 1], "count": n })
}

pub(super) fn mode(samples: &[Sample]) -> Value {
    let summary: BTreeMap<_, _> = samples
        .first()
        .into_iter()
        .flat_map(|s| s.keys())
        .map(|key| {
            (
                *key,
                statistics(samples.iter().filter_map(|s| s[key]).collect()),
            )
        })
        .collect();
    json!({"summary": summary, "samples_ms": samples})
}

pub(super) fn print(report: &Value, json_output: bool) -> Result<(), serde_json::Error> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "engine_bench: {} ({}x{}, DPR 1)",
            report["input"], report["viewport_css_px"][0], report["viewport_css_px"][1]
        );
        println!("Intervals may overlap; see docs/engine-bench.md. All times: ms.");
        if let Some(modes) = report["modes"].as_object() {
            for (name, mode) in modes {
                println!("{name:>5}  {:>28} {:>12} {:>12}", "scope", "median", "p90");
                if let Some(summary) = mode["summary"].as_object() {
                    for (scope, stats) in summary {
                        if let (Some(median), Some(p90)) =
                            (stats["median_ms"].as_f64(), stats["p90_ms"].as_f64())
                        {
                            println!("       {scope:>28} {median:12.4} {p90:12.4}");
                        } else {
                            println!("       {scope:>28}     not executed");
                        }
                    }
                }
            }
        }
        println!(
            "Process peak working set: {} bytes",
            report["peak_working_set_bytes"]
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_even_and_nearest_rank_p90_preserve_outliers() {
        let stats = statistics(vec![100., 2., 4., 1.]);
        assert_eq!(stats["median_ms"], 3.);
        assert_eq!(stats["p90_ms"], 100.);
        assert_eq!(statistics(vec![2.])["p90_ms"], 2.);
        assert!(statistics(vec![]).is_null());
    }

    #[test]
    fn skipped_scope_is_null_rather_than_a_zero_measurement() {
        let report = mode(&[Sample::from([("parse", None), ("paint", Some(0.5))])]);
        assert!(report["summary"]["parse"].is_null());
        assert!(report["samples_ms"][0]["parse"].is_null());
    }
}
