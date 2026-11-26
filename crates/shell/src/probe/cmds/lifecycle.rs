//! In-process acceptance controls; no process termination or OS input.

use serde_json::{Value, json};

pub(crate) fn handle(cmd: &str, req: &Value) -> Option<Value> {
    match cmd {
        "hostDisconnect" => Some(disconnect(
            crate::host_link::client().map(|client| move || client.disconnect()),
        )),
        "keystroke" => Some(crate::probe::keyboard::keystroke(req)),
        "filedrop" => Some(crate::probe::file_drop::file_drop(req)),
        // {"cmd":"damageAudit","on":true,"trace":true}: every partially drawn
        // frame is compared with a whole redraw, mismatches are logged as
        // errors; `trace` also logs the damaged tiles of every frame.
        "damageAudit" => {
            if let Some(on) = req.get("on").and_then(Value::as_bool) {
                gpui_windows::set_damage_audit(on);
            }
            if let Some(on) = req.get("trace").and_then(Value::as_bool) {
                crate::web::trace_frames(on);
            }
            Some(json!({"ok": true, "mismatches": gpui_windows::damage_audit_mismatches()}))
        }
        _ => None,
    }
}

fn disconnect(close: Option<impl FnOnce()>) -> Value {
    match close {
        Some(close) => {
            close();
            json!({"ok": true})
        }
        None => json!({"ok": false, "err": "host client not connected"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_disconnect_closes_the_current_client_once() {
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            disconnect(Some(|| calls.set(calls.get() + 1))),
            json!({"ok": true})
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(disconnect(None::<fn()>)["ok"], false);
    }

    #[test]
    fn routes_keystroke_and_rejects_missing_key() {
        let reply = handle("keystroke", &json!({})).unwrap();
        assert_eq!(reply["ok"], false);
        assert!(reply["err"].as_str().unwrap().contains("key is required"));
        assert!(handle("unrelated", &json!({})).is_none());
    }
}
