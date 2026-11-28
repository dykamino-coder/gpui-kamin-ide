//! Scroll animation and document window frame counters.

use serde_json::Value;
use std::fmt::Write;

pub(super) fn animation(s: &mut String, current: &Value, before: &Value) {
    let count = |key: &str| current[key].as_u64().unwrap_or(0) + before[key].as_u64().unwrap_or(0);
    if count("frames") > 0 {
        let _ = writeln!(
            s,
            "  scroll animation frames {} / builds {}",
            count("frames"),
            count("builds")
        );
    }
}

pub(super) fn window_frames(s: &mut String, current: &Value, before: &Value) {
    let count = |key: &str| current[key].as_u64().unwrap_or(0) + before[key].as_u64().unwrap_or(0);
    if count("frames") > 0 || count("builds") > 0 {
        let _ = writeln!(
            s,
            "  window frames {} / document builds {}",
            count("frames"),
            count("builds")
        );
    }
}
