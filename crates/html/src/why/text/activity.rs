//! Platform activity and externally sampled idle CPU in the human journal.

use serde_json::Value;
use std::fmt::Write;

fn activity(out: &mut String, value: &Value) {
    if !value.is_object() {
        return;
    }
    let _ = writeln!(
        out,
        "  platform callbacks={} draws={} presentations={} skips={} reentrant={} lost={}",
        value["platform_callbacks"],
        value["draws"],
        value["presentations"],
        value["frame_skips"],
        value["reentrant_callbacks"],
        value["foreground_entries_lost"]
    );
    for (reason, count) in value["frame_skip_reasons"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let _ = writeln!(out, "  platform skip {reason}={count}");
    }
}

pub(super) fn render(out: &mut String, report: &Value) {
    activity(out, &report["observations"]["platform_activity"]);
    for sample in report["idle_samples"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "idle quiet={} cpu_ms={} journal_ms={} process_cycles={} kernel_100ns={} user_100ns={}",
            sample["quiet"],
            sample["cpu_duration_ms"],
            sample["journal_duration_ms"],
            sample["process_cycles"],
            sample["process_kernel_100ns"],
            sample["process_user_100ns"]
        );
        activity(out, sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_idle_is_absent_and_nonzero_raw_cycles_are_not_rounded_away() {
        let mut text = String::new();
        render(&mut text, &json!({"frames": []}));
        assert!(text.is_empty());
        render(
            &mut text,
            &json!({"idle_samples": [{
                "quiet": false, "cpu_duration_ms": 5000.0, "journal_duration_ms": 6000.0,
                "process_cycles": 1, "process_kernel_100ns": 0, "process_user_100ns": 0,
                "platform_callbacks": 2, "draws": 0, "presentations": 0, "frame_skips": 2,
                "frame_skip_reasons": {"no_render_needed": 2}, "reentrant_callbacks": 0,
                "foreground_entries_lost": 0,
            }]}),
        );
        assert!(text.contains("idle quiet=false cpu_ms=5000.0 journal_ms=6000.0 process_cycles=1"));
        assert!(text.contains("callbacks=2 draws=0 presentations=0 skips=2 reentrant=0 lost=0"));
        assert!(text.contains("platform skip no_render_needed=2"));
    }
}
