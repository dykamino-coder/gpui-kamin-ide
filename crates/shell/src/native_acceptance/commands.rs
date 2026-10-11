//! Probe transport validates controls, then routes UI work through ShellEvent.

use super::Action;
use serde_json::{Value, json};

pub(crate) fn handle(cmd: &str, req: &Value) -> Option<Value> {
    if cmd != "nativeAcceptance" {
        return None;
    }
    let action = match req["action"].as_str() {
        Some("status") => {
            super::enable();
            return Some(json!({"ok": true, "debug": true, "wait": super::faults::status()}));
        }
        Some("wait") => {
            let status = match req["status"].as_str() {
                Some("timeout") => 258,
                Some("abandoned") => 128,
                _ => {
                    return Some(
                        json!({"ok": false, "err": "status must be timeout or abandoned"}),
                    );
                }
            };
            let count = req.get("count").and_then(Value::as_u64).unwrap_or(1);
            let view = req["view"].as_str().unwrap_or_default();
            let ok = u8::try_from(count).is_ok_and(|count| super::faults::arm(view, status, count));
            if ok {
                super::enable();
                crate::web::repaint_requested();
            }
            return Some(json!({"ok": ok, "wait": super::faults::status()}));
        }
        Some("loaders") => Action::Loaders,
        Some("waiting") => Action::Waiting,
        Some("ready") => Action::Ready,
        Some("error") => Action::Error,
        Some("switch") => Action::Switch,
        Some("clear") => Action::Clear,
        Some("deviceLoss") => Action::DeviceLoss,
        _ => return Some(json!({"ok": false, "err": "unknown acceptance action"})),
    };
    super::enable();
    Some(match crate::host_link::event_tx() {
        Some(tx) => match tx.try_send(crate::host_link::ShellEvent::NativeAcceptance(action)) {
            Ok(()) => json!({"ok": true, "queued": true}),
            Err(_) => json!({"ok": false, "err": "event channel closed"}),
        },
        None => json!({"ok": false, "err": "event channel not ready"}),
    })
}
