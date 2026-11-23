//! CPU paint attribution: inclusive intervals overlap and must not be summed.

use serde_json::Value;
use std::fmt::Write;

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
