//! Exclusive request_layout cost of a sample by element type (gpui
//! instrumentation): self-time outside child elements and the style copies
//! made there. Self-times add up to the measured part of the phase; each one
//! also carries the profiler's own cost for that element.

use gpui::prepaint_prof::ElementCost;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Element types kept per sample.
const KINDS: usize = 24;

pub(super) fn to_json(costs: Vec<(&'static str, ElementCost)>) -> Value {
    if costs.is_empty() {
        return Value::Null;
    }
    let mut merged = BTreeMap::<String, ElementCost>::new();
    for (name, cost) in costs {
        let entry = merged.entry(short(name)).or_default();
        entry.self_ns += cost.self_ns;
        entry.calls += cost.calls;
        for (own, add) in entry.self_costs.iter_mut().zip(cost.self_costs) {
            *own += add;
        }
    }
    let total: u64 = merged.values().map(|c| c.self_ns).sum();
    let mut list: Vec<_> = merged.into_iter().collect();
    list.sort_by(|a, b| b.1.self_ns.cmp(&a.1.self_ns).then_with(|| a.0.cmp(&b.0)));
    let kinds: Vec<_> = list
        .iter()
        .take(KINDS)
        .map(|(kind, c)| {
            json!({
                "kind": kind, "self_ns": c.self_ns, "calls": c.calls,
                "style_group_copies": c.self_costs[0], "style_group_copy_bytes": c.self_costs[1],
                "computed_copies": c.self_costs[2], "computed_copy_bytes": c.self_costs[3],
            })
        })
        .collect();
    // Every element whose request_layout ran: the elements the sample built.
    let calls: u64 = list.iter().map(|(_, c)| c.calls).sum();
    json!({"self_ns_total": total, "calls_total": calls, "kinds_total": list.len(), "top_by_self_ns": kinds})
}

/// The type name without module paths, generic arguments included.
fn short(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut word = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == ':' {
            word.push(ch);
        } else {
            out.push_str(word.rsplit("::").next().unwrap_or(""));
            word.clear();
            out.push(ch);
        }
    }
    out.push_str(word.rsplit("::").next().unwrap_or(""));
    out
}

pub(super) fn render(out: &mut String, value: &Value) {
    let Some(kinds) = value["top_by_self_ns"].as_array() else {
        return;
    };
    let total = value["self_ns_total"].as_u64().unwrap_or(0) as f64 / 1e6;
    let _ = writeln!(
        out,
        "  request_layout self-time by element type ({total:.3} ms measured, {} elements):",
        value["calls_total"]
    );
    for k in kinds {
        let _ = writeln!(
            out,
            "    {}: {:.3} ms calls={} group copies={} ({} B) computed copies={} ({} B)",
            k["kind"].as_str().unwrap_or("?"),
            k["self_ns"].as_u64().unwrap_or(0) as f64 / 1e6,
            k["calls"],
            k["style_group_copies"],
            k["style_group_copy_bytes"],
            k["computed_copies"],
            k["computed_copy_bytes"],
        );
    }
}

#[cfg(test)]
mod tests {
    use gpui::prepaint_prof::ElementCost;

    #[test]
    fn kinds_merge_by_short_name_and_sort_by_self_time() {
        let cost = |ns, copies| ElementCost {
            self_ns: ns,
            calls: 1,
            self_costs: [copies, copies * 8, 0, 0],
        };
        let value = super::to_json(vec![
            ("a::Hooked<b::Probe>", cost(5, 1)),
            ("c::Hooked<d::Probe>", cost(7, 2)),
            ("gpui::Div", cost(9, 0)),
        ]);
        assert_eq!(value["self_ns_total"], 21);
        assert_eq!(value["kinds_total"], 2);
        assert_eq!(value["calls_total"], 3);
        let top = &value["top_by_self_ns"];
        assert_eq!(top[0]["kind"], "Hooked<Probe>");
        assert_eq!(
            (&top[0]["self_ns"], &top[0]["calls"]),
            (&12.into(), &2.into())
        );
        assert_eq!(top[0]["style_group_copy_bytes"], 24);
        assert_eq!(top[1]["kind"], "Div");
        assert!(super::to_json(Vec::new()).is_null());
        let mut out = String::new();
        super::render(&mut out, &value);
        assert!(out.contains("Hooked<Probe>: 0.000 ms calls=2 group copies=3"));
    }
}
