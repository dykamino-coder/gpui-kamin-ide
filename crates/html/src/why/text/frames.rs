//! Frame request explanation.
use super::text;
use serde_json::Value;
use std::fmt::Write;

/// The frame events in order, repeats of one event in a row as `xN`, and a
/// summary of draws and renders with the requests after the first render
/// (who asked for a frame beyond the expected one).
pub(super) fn frames(s: &mut String, label: &str, events: &Value, dropped: &Value) {
    let all = events.as_array().map_or(&[][..], Vec::as_slice);
    if all.is_empty() {
        return;
    }
    let count = |kind: &str| all.iter().filter(|e| e["kind"] == kind).count();
    let first_render = all.iter().position(|e| e["kind"] == "render");
    let late: Vec<String> = first_render
        .map(|at| &all[at + 1..])
        .unwrap_or_default()
        .iter()
        .filter(|e| {
            e["kind"] != "draw"
                && e["kind"] != "render"
                && !text(&e["kind"]).starts_with("scroll_node.")
        })
        .map(|e| format!("{} at {}", text(&e["kind"]), text(&e["site"])))
        .collect();
    let _ = write!(
        s,
        "{label} draws={} renders={}",
        count("draw"),
        count("render")
    );
    if !late.is_empty() {
        let _ = write!(s, "; requested after the first render: {}", late.join(", "));
    }
    let _ = writeln!(s);
    let mut at = 0;
    while at < all.len() {
        let run = all[at..].iter().take_while(|e| *e == &all[at]).count();
        let times = if run > 1 {
            format!(" x{run}")
        } else {
            String::new()
        };
        let detail = text(&all[at]["detail"]);
        let sep = if detail.is_empty() { "" } else { ": " };
        let _ = writeln!(
            s,
            "{label}   {}{times} at {}{sep}{detail}",
            text(&all[at]["kind"]),
            text(&all[at]["site"])
        );
        at += run;
    }
    if dropped.as_u64().unwrap_or(0) > 0 {
        let _ = writeln!(s, "{label}   ... {dropped} more not kept");
    }
}
