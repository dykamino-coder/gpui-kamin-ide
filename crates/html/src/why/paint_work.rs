//! CPU paint attribution: inclusive intervals overlap and must not be summed.

use serde_json::Value;
use std::fmt::Write;

/// A frame's paint groups by reason, then its CPU paint attribution.
pub(super) fn render_frame(out: &mut String, frame: &Value) {
    let reasons: Vec<String> = frame["group_reasons"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, n)| format!("{k}={n}"))
        .collect();
    if !reasons.is_empty() {
        let _ = writeln!(out, "  groups by reason: {}", reasons.join(" "));
    }
    render(out, &frame["paint_work"]);
}

pub(super) fn render(out: &mut String, profile: &Value) {
    let Some(intervals) = profile["intervals_inclusive_ns"].as_object() else {
        return;
    };
    let _ = writeln!(
        out,
        "  paint CPU intervals (inclusive, overlapping; do not sum):"
    );
    for (name, ns) in intervals {
        if let Some(ns) = ns.as_u64().filter(|ns| *ns != 0) {
            let _ = writeln!(out, "    {name}: {:.3} ms", ns as f64 / 1_000_000.0);
        }
    }
    let _ = writeln!(
        out,
        "    gradient rasters={} pixels={} rounded masks={}",
        profile["gradient_rasters"],
        profile["gradient_raster_pixels"],
        profile["rounded_mask_rasters"]
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn groups_by_reason_print_on_one_line_and_none_print_nothing() {
        let mut out = String::new();
        super::render_frame(&mut out, &serde_json::json!({"group_reasons": {}}));
        assert!(out.is_empty());
        super::render_frame(
            &mut out,
            &serde_json::json!({"group_reasons": {"rounded_mask": 3, "blur": 1}}),
        );
        assert!(out.starts_with("  groups by reason: ") && out.ends_with('\n'));
        assert!(out.contains("rounded_mask=3") && out.contains("blur=1"));
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn intervals_are_labelled_inclusive_and_absence_is_not_zero() {
        let mut out = String::new();
        super::render(&mut out, &serde_json::Value::Null);
        assert!(out.is_empty());
        super::render(
            &mut out,
            &serde_json::json!({"intervals_inclusive_ns": {"gradient_raster_ns": 1234567}, "gradient_rasters": 2, "gradient_raster_pixels": 500, "rounded_mask_rasters": 0}),
        );
        assert!(out.contains("overlapping; do not sum"));
        assert!(out.contains("gradient_raster_ns: 1.235 ms"));
        assert!(out.contains("rasters=2 pixels=500"));
    }
}
