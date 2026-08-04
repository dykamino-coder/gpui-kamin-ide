//! Инъекции probe: события файлов, вида и раскладки.
//!
//! Ветки перенесены как есть (`plan/100-refactor-250.md`).

use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use serde_json::Value;

/// Файлы, режимы панелей, тултипы и раскладка. `None` — не наша группа.
pub(crate) fn emit_view(
    kind: &str,
    req: &Value,
    sid: String,
    name_arg: String,
) -> Option<ShellEvent> {
    let name = name_arg;
    let _ = (&sid, &name);
    Some(match kind {
        "fileMode" => ShellEvent::Ed(EdEvent::SetFileMode(if name == "web" {
            "web"
        } else {
            "files"
        })),
        "openFile" => match req.get("line").and_then(Value::as_u64) {
            Some(line) => ShellEvent::Ed(EdEvent::OpenFileAt(name, line as u32)),
            None => ShellEvent::Ed(EdEvent::OpenFile(name)),
        },
        // {"cmd":"emit","kind":"tooltip","name":"Текст","x":..,"y":..} —
        // показать тултип без ховера (проверка overlay-ветки)
        "tooltip" => ShellEvent::TooltipShow(
            if name.is_empty() {
                "Tooltip".to_string()
            } else {
                name.clone()
            },
            req.get("x").and_then(Value::as_f64).unwrap_or(200.0) as f32,
            req.get("y").and_then(Value::as_f64).unwrap_or(200.0) as f32,
        ),
        _ => return None,
    })
}
