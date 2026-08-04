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
