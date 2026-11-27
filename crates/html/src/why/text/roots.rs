//! Human-readable root identity and non-overlapping phase attribution.

use serde_json::Value;
use std::fmt::Write;

pub(super) fn render(out: &mut String, roots: &Value, label: &str) {
    for root in roots["attempts"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "  {label} document={} tree={} attempt={} prepared={} painted={} total={:.3} ms",
            root["document"],
            root["tree"],
            root["attempt"],
            root["prepared"],
            root["painted"],
            root["exclusive_total_ms"].as_f64().unwrap_or(0.)
        );
        for (name, phase) in root["phases_ms"].as_object().into_iter().flatten() {
            let _ = writeln!(
                out,
                "    {name} inclusive={:.3} exclusive={:.3} calls={}",
                phase["inclusive"].as_f64().unwrap_or(0.),
                phase["exclusive"].as_f64().unwrap_or(0.),
                phase["calls"]
            );
        }
        if root["pending_host_layout"] == true {
            let _ = writeln!(out, "    host layout incomplete at checkpoint");
        }
    }
    if roots["dropped"].as_u64().unwrap_or(0) > 0 {
        let _ = writeln!(out, "  {label} ... {} attempts not kept", roots["dropped"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roots_keep_identity_nested_costs_and_incomplete_attempts_visible() {
        let mut out = String::new();
        render(
            &mut out,
            &json!({"attempts": [{"document": 3, "tree": 4, "attempt": 5,
            "prepared": true, "painted": false, "exclusive_total_ms": 7.,
            "phases_ms": {"build": {"inclusive": 2., "exclusive": 2., "calls": 1}},
            "pending_host_layout": true}], "dropped": 1}),
            "root",
        );
        assert!(
            out.contains("document=3 tree=4 attempt=5 prepared=true painted=false total=7.000 ms")
        );
        assert!(out.contains("build inclusive=2.000 exclusive=2.000 calls=1"));
        assert!(out.contains("host layout incomplete at checkpoint"));
        assert!(out.contains("1 attempts not kept"));
    }
}
