//! CPU layout attribution: inclusive intervals overlap and must not be summed;
//! repeated layout units are the upper bound of a layout cache by inputs.

use serde_json::Value;
use std::fmt::Write;

pub(super) fn render(out: &mut String, profile: &Value) {
    let Some(intervals) = profile["intervals_inclusive_ns"].as_object() else {
        return;
    };
    let _ = writeln!(
        out,
        "  layout CPU intervals (inclusive, overlapping; do not sum):"
    );
    for (name, ns) in intervals {
        if let Some(ns) = ns.as_u64().filter(|ns| *ns != 0) {
            let _ = writeln!(out, "    {name}: {:.3} ms", ns as f64 / 1_000_000.0);
        }
    }
    let c = &profile["counts"];
    let _ = writeln!(
        out,
        "    layout units={} repeated in frame={} same as last frame={}; probes={} shaped={}/{} split={}/{}",
        c["layout_units"],
        c["layout_units_repeated_in_frame"],
        c["layout_units_same_as_last_frame"],
        c["band_probe_calls"],
        c["measure_cache_misses"],
        c["measure_cache_hits"],
        c["layout_cache_misses"],
        c["layout_cache_hits"],
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn layout_intervals_are_inclusive_and_absence_is_not_zero() {
        let mut out = String::new();
        super::render(&mut out, &serde_json::Value::Null);
        assert!(out.is_empty());
        super::render(
            &mut out,
            &serde_json::json!({"intervals_inclusive_ns": {"line_break_ns": 2500000}, "counts": {"layout_units": 4, "layout_units_same_as_last_frame": 1}}),
        );
        assert!(out.contains("overlapping; do not sum"));
        assert!(out.contains("line_break_ns: 2.500 ms"));
        assert!(out.contains("layout units=4"));
        assert!(out.contains("same as last frame=1"));
    }
}
