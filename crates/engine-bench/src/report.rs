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

/// Режим с замерами, счётчиками работы и ресурсами процесса каждого образца.
/// Один формат для cold/warm/relayout и edit: без него edit-фазы не судимы по
/// памяти и CPU (verdict.py читает `process_resources`).
pub(super) fn measured_mode(
    samples: &[Sample],
    diagnostics: &[Sample],
    work: Vec<Value>,
    usage: Vec<Value>,
) -> Value {
    let mut out = mode(samples);
    out["diagnostic_scopes"] = mode(diagnostics);
    out["work_counters"] = json!({"samples": work});
    out["process_resources"] = json!({"samples": usage});
    out
}

pub(super) fn print(report: &Value, json_output: bool) -> Result<(), serde_json::Error> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "engine_bench: {} ({}x{}, DPR 1)",
            report["input"], report["viewport_css_px"][0], report["viewport_css_px"][1]
        );
        println!("Phases partition each sample total; medians need not add up. All times: ms.");
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
        println!("frame_pixels_changed: {}", report["frame_pixels_changed"]);
        if let Some(changes) = report["frame_pixel_differences"].as_array() {
            for change in changes {
                println!(
                    "Frame {} ({} sample {}): {} changed RGB pixels vs frame 1, bounds_xywh={}",
                    change["frame_index"],
                    change["mode"],
                    change["sample_index"],
                    change["different_rgb_pixels"],
                    change["bounds_xywh"]
                );
            }
        }
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

    #[test]
    fn every_measured_mode_carries_per_sample_work_and_process_resources() {
        let samples = [
            Sample::from([("paint", Some(0.5))]),
            Sample::from([("paint", Some(0.7))]),
        ];
        let usage = vec![json!({"phases": {"paint": {"process_cpu_100ns": 0}}}); 2];
        let work = vec![json!({"phases": {}}); 2];
        let mode = measured_mode(&samples, &samples, work, usage);
        assert_eq!(
            mode["process_resources"]["samples"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            mode["work_counters"]["samples"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(mode["diagnostic_scopes"]["summary"]["paint"]["count"], 2);
        assert_eq!(mode["summary"]["paint"]["count"], 2);
    }
}
