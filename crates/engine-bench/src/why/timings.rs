//! Readable startup and nested-pass intervals, separate from disjoint phases.

use serde_json::Value;
use std::fmt::Write;

pub(super) fn render(out: &Value) -> String {
    let mut text = String::new();
    if let Some(cold) = out["process_cold"].as_object() {
        let _ = writeln!(
            text,
            "\nprocess cold: {}",
            cold["scope"].as_str().unwrap_or("")
        );
        let _ = writeln!(text, "  {}", cold["clock"].as_str().unwrap_or(""));
        let _ = writeln!(
            text,
            "  present: {}",
            cold["present"].as_str().unwrap_or("")
        );
        for (name, value) in cold {
            if let Some(ms) = value.as_f64() {
                let _ = writeln!(text, "  {name}={ms:.3}");
            }
        }
        if let Some(stages) = cold["host_stages_ms"].as_object() {
            for (name, ms) in stages {
                let _ = writeln!(text, "  host {name}={ms}");
            }
        }
        for (name, value) in cold["native_intervals_ms"]
            .as_object()
            .into_iter()
            .flatten()
        {
            let _ = writeln!(
                text,
                "  native {name}: calls={} inclusive_ms={}",
                value["calls"], value["inclusive_ms"]
            );
        }
    }
    for frame in out["frames"].as_array().into_iter().flatten() {
        for (name, value) in frame["native_intervals_ms"]
            .as_object()
            .into_iter()
            .flatten()
        {
            let _ = writeln!(
                text,
                "  native #{} {name}: calls={} inclusive_ms={}",
                frame["frame"], value["calls"], value["inclusive_ms"]
            );
        }
        for (name, value) in frame["timings_ms"].as_object().into_iter().flatten() {
            let _ = writeln!(
                text,
                "  interval #{} {name}: calls={} inclusive_ms={}",
                frame["frame"], value["calls"], value["inclusive_ms"]
            );
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn old_journals_remain_readable_and_new_intervals_keep_their_scope() {
        assert!(render(&json!({"frames": []})).is_empty());
        let rendered = render(&json!({
            "process_cold": {"scope": "whole process", "clock": "bridge", "process_to_capture_ms": 25., "host_stages_ms": {"device": 8.}},
            "frames": [{"frame": 1, "timings_ms": {"fonts": {"calls": 2, "inclusive_ms": 3.}}}],
        }));
        assert!(rendered.contains("process_to_capture_ms=25.000"));
        assert!(rendered.contains("host device=8.0"));
        assert!(rendered.contains("interval #1 fonts: calls=2 inclusive_ms=3.0"));
    }
}
