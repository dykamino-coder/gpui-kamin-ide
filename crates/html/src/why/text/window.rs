//! Embedded window presentation fields; absent observations remain absent.

use serde_json::Value;
use std::fmt::Write;

pub(super) fn render(out: &mut String, frame: &Value) {
    if let Some(inside) = frame["builds_inside_draw"].as_u64() {
        let _ = writeln!(out, "  builds inside draw={inside}");
    }
    let times: Vec<_> = [
        "first_present_ms",
        "present_interval_ms",
        "raster_submit_ms",
    ]
    .into_iter()
    .filter_map(|name| {
        frame[name]
            .as_f64()
            .map(|value| format!("{name}={value:.3}"))
    })
    .collect();
    if let Some(presented) = frame["presented"].as_bool() {
        let _ = writeln!(out, "  presented={presented} {}", times.join(" "));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_presentation_is_not_reported_as_zero() {
        let mut out = String::new();
        super::render(&mut out, &serde_json::json!({"built": 1}));
        assert!(out.is_empty());
        super::render(
            &mut out,
            &serde_json::json!({"presented": true,
            "builds_inside_draw": 0, "present_interval_ms": 16.5, "first_present_ms": null}),
        );
        assert!(out.contains("builds inside draw=0"));
        assert!(out.contains("present_interval_ms=16.500"));
        assert!(!out.contains("first_present_ms="));
    }
}
