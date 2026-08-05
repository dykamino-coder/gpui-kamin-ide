//! Инъекции probe: события сессий и оверлеев.
//!
//! Ветки перенесены как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::ui::context_menu::SessionMenuData;
use crate::ui::modal::{Modal, ModalAction};
use serde_json::Value;

/// События сессий, меню, модалок и тостов. `None` — не наша группа.
pub(crate) fn emit_session(
    kind: &str,
    req: &Value,
    sid: String,
    name_arg: String,
) -> Option<ShellEvent> {
    let name = name_arg;
    let _ = (&sid, &name);
    Some(match kind {
        // {"cmd":"emit","kind":"activateSession","id":"<sessionId>"} —
        // переключение сессии как кликом по строке сайдбара (стенды свитча)
        "activateSession" => ShellEvent::ActivateSession(sid),
        "sessionMenu" => {
            let x = req.get("x").and_then(Value::as_f64).unwrap_or(320.0) as f32;
            let y = req.get("y").and_then(Value::as_f64).unwrap_or(200.0) as f32;
            ShellEvent::OpenSessionMenu(
                SessionMenuData {
                    id: sid,
                    name,
                    open: req.get("open").and_then(Value::as_bool).unwrap_or(true),
                    pinned: req.get("pinned").and_then(Value::as_bool).unwrap_or(false),
                    color: req.get("color").and_then(Value::as_str).map(String::from),
                },
                x,
                y,
            )
        }
        // "body" переопределяет текст — так проверяется разметка тела
        // (`<b>`/`<code>`/`<br>`) без похода в реальный сценарий
        "confirm" => ShellEvent::OpenModal(Modal {
            title: "Delete session?".into(),
            body: req
                .get("body")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| {
                    format!(
                        "Session <strong>{name}</strong> will be removed. This cannot be undone."
                    )
                })
                .into(),
            confirm_label: "Delete".into(),
            danger: true,
            prompt: None,
            placeholder: None,
            validate: None,
            cancel_label: None,
            action: ModalAction::DeleteSession(sid),
        }),
        "rename" => ShellEvent::BeginRename(sid),
        "hoverPill" => if sid == "probe-session" {
            ShellEvent::DismissHoverPill
        } else {
            ShellEvent::HoverPill {
                id: sid,
                source: crate::host_link::HoverPillSource::Anchor,
                hovered: true,
            }
        },
        "toast" => ShellEvent::Toast(crate::ui::toasts::Toast {
            id: "probe-toast".into(),
            // Агенты шлют и `severity`, и `level` — принимаем оба, иначе
            // тестовый тост «Compile error» приезжал с ЗЕЛЁНОЙ галкой
            severity: req
                .get("severity")
                .or_else(|| req.get("level"))
                .and_then(Value::as_str)
                .unwrap_or("info")
                .to_string(),
            title: req.get("title").and_then(Value::as_str).map(String::from),
            message: if name == "Probe session" {
                "Build finished in 2.3s".to_string()
            } else {
                name
            },
            actions: req
                .get("actions")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            // По умолчанию тост САМ уходит: `sticky: true` намертво вешал
            // пробный тост в углу до перезапуска приложения
            sticky: req.get("sticky").and_then(Value::as_bool).unwrap_or(false),
        }),
        _ => return None,
    })
}
