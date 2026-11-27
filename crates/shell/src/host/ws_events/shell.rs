//! Уведомления, канал Output и смена воркспейса.
//!
//! Ветки перенесены дословно из `connect_ws` (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use serde_json::Value;
use smol::channel::Sender;

use crate::host_link::ShellEvent;

/// Имя параметра `on_event_tx` сохранено из замыкания: тела веток
/// перенесены дословно. `true` — канал обработан здесь.
pub(crate) fn handle(on_event_tx: &Sender<ShellEvent>, channel: &str, payload: &Value) -> bool {
    match channel {
        "kamin:editor:openLocation" => {
            let coordinate = |key: &str| {
                u32::try_from(payload.get(key)?.as_u64()?)
                    .ok()
                    .filter(|n| *n > 0)
            };
            if let (Some(path), Some(line), Some(column)) = (
                payload.get("path").and_then(Value::as_str),
                coordinate("line"),
                coordinate("column"),
            ) {
                let _ = on_event_tx.try_send(ShellEvent::Ed(
                    crate::host::events::EdEvent::OpenTextLocation(path.to_owned(), line, column),
                ));
            }
        }
        "kamin:notification:show" => {
            let id = payload
                .get("id")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| {
                    let ns = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    format!("toast-{ns}")
                });
            let toast = crate::ui::toasts::Toast {
                id,
                severity: payload
                    .get("severity")
                    .and_then(Value::as_str)
                    .unwrap_or("info")
                    .to_string(),
                title: payload
                    .get("title")
                    .and_then(Value::as_str)
                    .map(String::from),
                message: payload
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                actions: payload
                    .get("actions")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default(),
                duration_ms: None,
                sticky: payload
                    .get("sticky")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            };
            let _ = on_event_tx.try_send(ShellEvent::Toast(toast));
        }
        // Output-каналы (VS Code Output): append/replace/clear/dispose/show
        // env.clipboard.writeText расширения (кнопки «копировать» в
        // страницах Bridge): DOM-путь в вебвью закрыт, пишет ХОСТ — мы.
        "kamin:clipboard:write" => {
            let text = payload.as_str().unwrap_or_default().to_string();
            let _ = on_event_tx.try_send(ShellEvent::Ed(
                crate::host::events_editor::EdEvent::CopyToClipboard(text),
            ));
        }
        "kamin:output:event" => {
            let ext = payload
                .get("extensionId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let channel = payload
                .get("channel")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let op = payload
                .get("op")
                .and_then(Value::as_str)
                .unwrap_or("append")
                .to_string();
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .map(String::from);
            let _ =
                on_event_tx.try_send(ShellEvent::Cz(CzEvent::OutputEvent(ext, channel, op, text)));
        }
        // Активация сессии: workspace:changed приходит ПЕРВЫМ (plan/50 §5)
        "kamin:workspace:changed" => {
            let path = payload.as_str().map(String::from);
            let _ = on_event_tx.try_send(ShellEvent::Workspace(path));
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_editor_location_keeps_both_coordinates() {
        let (tx, rx) = smol::channel::unbounded();
        let channel = "kamin:editor:openLocation";
        let mut payload =
            serde_json::json!({"path":"C:/Fixture/sample file.ts","line":4,"column":2});
        assert!(handle(&tx, channel, &payload));
        assert!(
            matches!(rx.try_recv(), Ok(ShellEvent::Ed(crate::host::events::EdEvent::OpenTextLocation(path, 4, 2))) if path == "C:/Fixture/sample file.ts")
        );
        payload["column"] = serde_json::json!(0);
        assert!(handle(&tx, channel, &payload));
        assert!(rx.try_recv().is_err());
    }
}
