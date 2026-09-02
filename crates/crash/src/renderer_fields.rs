use std::collections::hash_map::RandomState;
use std::hash::BuildHasher as _;
use std::sync::OnceLock;

pub(crate) fn safe_status(status: &str) -> &'static str {
    if status.contains("TS_ABNORMAL_TERMINATION") {
        "abnormal"
    } else if status.contains("TS_PROCESS_WAS_KILLED") {
        "killed"
    } else if status.contains("TS_PROCESS_CRASHED") {
        "crashed"
    } else if status.contains("TS_PROCESS_OOM") {
        "oom"
    } else if status.contains("TS_LAUNCH_FAILED") {
        "launch-failed"
    } else if status.contains("TS_INTEGRITY_FAILURE") {
        "integrity-failure"
    } else {
        "unknown"
    }
}

pub(crate) fn view_ref(view_id: &str) -> String {
    let kind = if view_id == "claudeBridgeChat" {
        "claude-bridge-chat"
    } else if view_id.starts_with("claudeBridge") {
        "claude-bridge-view"
    } else if view_id == "browser" {
        "browser"
    } else {
        "contributed-view"
    };
    static HASHER: OnceLock<RandomState> = OnceLock::new();
    let hash = HASHER.get_or_init(RandomState::new).hash_one(view_id);
    format!("{kind}:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_allowlisted_and_view_ids_are_opaque() {
        let secret = "claudeBridgeCustom-secret-token-and-session";
        let view = view_ref(secret);
        assert!(view.starts_with("claude-bridge-view:"));
        assert!(!view.contains("secret"));
        assert_eq!(safe_status("TerminationStatus(TS_PROCESS_OOM)"), "oom");
        assert_eq!(safe_status("https://token.example"), "unknown");
    }
}
