//! Explicit owner snapshots are separate from per-frame observations.

use super::{Value, text};
use std::fmt::Write;

pub(super) fn frame(s: &mut String, memory: &Value) {
    if memory["sampled"] == false {
        let _ = writeln!(
            s,
            "  memory not sampled; latest snapshot {}, process private {} bytes",
            memory["sample_id"], memory["process_private_bytes"]
        );
    } else {
        // Read existing schema-1 reports whose owners were sampled per frame.
        owners(s, memory);
    }
}

pub(super) fn sample(s: &mut String, memory: &Value) {
    let _ = writeln!(
        s,
        "\nmemory snapshot {}: {} after frame {}, {:.3} ms",
        memory["sample_id"],
        text(&memory["reason"]),
        memory["after_frame"],
        memory["duration_ms"].as_f64().unwrap_or(0.)
    );
    owners(s, memory);
}

fn owners(s: &mut String, memory: &Value) {
    let _ = writeln!(
        s,
        "  memory heap owners {} bytes, gpu {} bytes, process private {} bytes, outside the owners' report {} bytes",
        memory["heap_bytes"],
        memory["gpu_bytes"],
        memory["process_private_bytes"],
        memory["outside_owners_bytes"]
    );
    for change in memory["changed_bytes"]
        .as_array()
        .into_iter()
        .flatten()
        .take(8)
    {
        let _ = writeln!(
            s,
            "  memory {:+} {}",
            change[1].as_i64().unwrap_or(0),
            text(&change[0])
        );
    }
}
